use core::{cmp, iter::Step, mem::size_of, ops::Add};

use x86_64::{
    align_up,
    structures::paging::{
        mapper::{MappedFrame, MapperAllSizes, TranslateResult},
        FrameAllocator, Page, PageSize, PageTableFlags as Flags, PhysFrame, Size4KiB, Translate,
    },
    PhysAddr, VirtAddr,
};

use xmas_elf::{
    dynamic, header,
    program::{self, ProgramHeader, SegmentData, Type},
    sections::Rela,
    ElfFile,
};

use synapse::tls_template::TlsTemplate;
use crate::entries::Entries;

const PAGE_SIZE: u64 = 4096;

#[derive(Clone, Copy)]
pub struct VirtualAddressOffset {
    virtual_address_offset: i128,
}

impl VirtualAddressOffset {
    pub fn zero() -> Self {
        Self::new(0)
    }

    pub fn new(virtual_address_offset: i128) -> Self {
        Self {
            virtual_address_offset,
        }
    }

    pub fn virtual_address_offset(&self) -> i128 {
        self.virtual_address_offset
    }
}

impl Add<u64> for VirtualAddressOffset {
    type Output = u64;

    fn add(self, offset: u64) -> Self::Output {
        u64::try_from(
            self.virtual_address_offset + i128::from(offset)
        )
            .unwrap()
    }
}

pub struct Kernel<'a> {
    pub elf: ElfFile<'a>,
    pub start_address: *const u8,
    pub len: usize,
}

impl<'a> Kernel<'a> {
    pub fn parse(kernel_slice: &'a [u8]) -> Self {
        let kernel_elf = ElfFile::new(kernel_slice).unwrap();
        Kernel {
            elf: kernel_elf,
            start_address: kernel_slice.as_ptr(),
            len: kernel_slice.len(),
        }
    }
}

const COPIED: Flags = Flags::BIT_9;

struct Loader<'a, M, F> {
    elf_file: ElfFile<'a>,
    inner: Inner<'a, M, F>,
}

struct Inner<'a, M, F> {
    kernel_offset: PhysAddr,
    virtual_address_offset: VirtualAddressOffset,
    page_table: &'a mut M,
    frame_allocator: &'a mut F,
}

fn check_is_in_load(elf_file: &ElfFile, virt_offset: u64) -> Result<(), &'static str> {
    for program_header in elf_file.program_iter() {
        if let Type::Load = program_header.get_type()? {
            if program_header.virtual_addr() <= virt_offset {
                let offset_in_segment = virt_offset - program_header.virtual_addr();
                if offset_in_segment < program_header.mem_size() {
                    return Ok(());
                }
            }
        }
    }
    Err("offset is not in load segment")
}

impl<'a, M, F> Inner<'a, M, F>
    where
        M: MapperAllSizes + Translate,
        F: FrameAllocator<Size4KiB>,
{
    fn handle_load_segment(&mut self, segment: ProgramHeader) -> Result<(), &'static str> {
        let phys_start_addr = self.kernel_offset + segment.offset();
        let start_frame: PhysFrame = PhysFrame::containing_address(phys_start_addr);
        let end_frame: PhysFrame =
            PhysFrame::containing_address(phys_start_addr + segment.file_size() - 1u64);

        let virt_start_addr = VirtAddr::new(self.virtual_address_offset + segment.virtual_addr());
        let start_page: Page = Page::containing_address(virt_start_addr);

        let mut segment_flags = Flags::PRESENT;
        if !segment.flags().is_execute() {
            segment_flags |= Flags::NO_EXECUTE;
        }
        if segment.flags().is_write() {
            segment_flags |= Flags::WRITABLE;
        }

        // map all frames of the segment at the desired virtual address
        for frame in PhysFrame::range_inclusive(start_frame, end_frame) {
            let offset = frame - start_frame;
            let page = start_page + offset;
            let flusher = unsafe {
                self.page_table
                    .map_to(page, frame, segment_flags, self.frame_allocator)
                    .map_err(|_err| "map_to failed")?
            };

            flusher.ignore();
        }

        // Handle .bss section (mem_size > file_size)
        if segment.mem_size() > segment.file_size() {
            self.handle_bss_section(&segment, segment_flags)?;
        }

        Ok(())
    }

    fn handle_bss_section(
        &mut self,
        segment: &ProgramHeader,
        segment_flags: Flags,
    ) -> Result<(), &'static str> {
        let virt_start_addr = VirtAddr::new(self.virtual_address_offset + segment.virtual_addr());
        let mem_size = segment.mem_size();
        let file_size = segment.file_size();

        // calculate virtual memory region that must be zeroed
        let zero_start = virt_start_addr + file_size;
        let zero_end = virt_start_addr + mem_size;

        type PageArray = [u64; Size4KiB::SIZE as usize / 8];
        const ZERO_ARRAY: PageArray = [0; Size4KiB::SIZE as usize / 8];

        let data_bytes_before_zero = zero_start.as_u64() & 0xfff;
        if data_bytes_before_zero != 0 {
            let last_page = Page::containing_address(virt_start_addr + file_size - 1u64);
            let new_frame = unsafe { self.make_mut(last_page) };
            let new_bytes_ptr = new_frame.start_address().as_u64() as *mut u8;

            unsafe {
                core::ptr::write_bytes(
                    new_bytes_ptr.add(data_bytes_before_zero as usize),
                    0,
                    (Size4KiB::SIZE - data_bytes_before_zero) as usize,
                );
            }
        }

        let start_page: Page =
            Page::containing_address(VirtAddr::new(align_up(zero_start.as_u64(), Size4KiB::SIZE)));
        let end_page = Page::containing_address(zero_end - 1u64);

        for page in Page::range_inclusive(start_page, end_page) {
            let frame = self.frame_allocator.allocate_frame().unwrap();

            let frame_ptr = frame.start_address().as_u64() as *mut PageArray;
            unsafe { frame_ptr.write(ZERO_ARRAY) };

            let flusher = unsafe {
                self.page_table
                    .map_to(page, frame, segment_flags, self.frame_allocator)
                    .map_err(|_err| "Failed to map new frame for bss memory")?
            };

            flusher.ignore();
        }

        Ok(())
    }

    fn copy_from(&self, addr: VirtAddr, buf: &mut [u8]) {
        let end_inclusive_addr = Step::forward_checked(addr, buf.len() - 1)
            .expect("end address outside of the virtual address space");
        let start_page = Page::<Size4KiB>::containing_address(addr);
        let end_inclusive_page = Page::<Size4KiB>::containing_address(end_inclusive_addr);

        for page in start_page..=end_inclusive_page {
            let phys_addr = self
                .page_table
                .translate_page(page)
                .expect("address is not mapped to the kernel's memory space");

            let page_start = page.start_address();
            let page_end_inclusive = page.start_address() + 4095u64;

            let start_copy_address = cmp::max(addr, page_start);
            let end_inclusive_copy_address = cmp::min(end_inclusive_addr, page_end_inclusive);

            let start_offset_in_frame = (start_copy_address - page_start) as usize;
            let end_inclusive_offset_in_frame = (end_inclusive_copy_address - page_start) as usize;

            let copy_len = end_inclusive_offset_in_frame - start_offset_in_frame + 1;

            let start_phys_addr = phys_addr.start_address() + start_offset_in_frame;

            let start_offset_in_buf = Step::steps_between(&addr, &start_copy_address).unwrap();

            let src_ptr = start_phys_addr.as_u64() as *const u8;
            let src = unsafe {
                &*core::ptr::slice_from_raw_parts(src_ptr, copy_len)
            };

            let dest = &mut buf[start_offset_in_buf..][..copy_len];

            dest.copy_from_slice(src);
        }
    }

    unsafe fn copy_to(&mut self, addr: VirtAddr, buf: &[u8]) {
        let end_inclusive_addr = Step::forward_checked(addr, buf.len() - 1)
            .expect("the end address should be in the virtual address space");
        let start_page = Page::<Size4KiB>::containing_address(addr);
        let end_inclusive_page = Page::<Size4KiB>::containing_address(end_inclusive_addr);

        for page in start_page..=end_inclusive_page {
            let phys_addr = unsafe {
                self.make_mut(page)
            };

            let page_start = page.start_address();
            let page_end_inclusive = page.start_address() + 4095u64;

            let start_copy_address = cmp::max(addr, page_start);
            let end_inclusive_copy_address = cmp::min(end_inclusive_addr, page_end_inclusive);

            let start_offset_in_frame = (start_copy_address - page_start) as usize;
            let end_inclusive_offset_in_frame = (end_inclusive_copy_address - page_start) as usize;

            let copy_len = end_inclusive_offset_in_frame - start_offset_in_frame + 1;

            let start_phys_addr = phys_addr.start_address() + start_offset_in_frame;

            let start_offset_in_buf = Step::steps_between(&addr, &start_copy_address).unwrap();

            let dest_ptr = start_phys_addr.as_u64() as *mut u8;
            let dest = unsafe {
                &mut *core::ptr::slice_from_raw_parts_mut(dest_ptr, copy_len)
            };

            let src = &buf[start_offset_in_buf..][..copy_len];

            dest.copy_from_slice(src);
        }
    }

    unsafe fn make_mut(&mut self, page: Page) -> PhysFrame {
        let (frame, flags) = match self.page_table.translate(page.start_address()) {
            TranslateResult::Mapped {
                frame,
                offset: _,
                flags,
            } => (frame, flags),
            TranslateResult::NotMapped => panic!("{:?} is not mapped", page),
            TranslateResult::InvalidFrameAddress(_) => unreachable!(),
        };
        let frame = if let MappedFrame::Size4KiB(frame) = frame {
            frame
        } else {
            unreachable!()
        };

        if flags.contains(COPIED) {
            return frame;
        }

        let new_frame = self.frame_allocator.allocate_frame().unwrap();
        let frame_ptr = frame.start_address().as_u64() as *const u8;
        let new_frame_ptr = new_frame.start_address().as_u64() as *mut u8;

        unsafe {
            core::ptr::copy_nonoverlapping(frame_ptr, new_frame_ptr, Size4KiB::SIZE as usize);
        }

        self.page_table.unmap(page).unwrap().1.ignore();
        let new_flags = flags | COPIED;

        unsafe {
            self.page_table
                .map_to(page, new_frame, new_flags, self.frame_allocator)
                .unwrap()
                .ignore();
        }

        new_frame
    }

    fn remove_copied_flags(&mut self, elf_file: &ElfFile) -> Result<(), &'static str> {
        for program_header in elf_file.program_iter() {
            if let Type::Load = program_header.get_type()? {
                let start = self.virtual_address_offset + program_header.virtual_addr();
                let end = start + program_header.mem_size();
                let start = VirtAddr::new(start);
                let end = VirtAddr::new(end);
                let start_page = Page::containing_address(start);
                let end_page = Page::containing_address(end - 1u64);
                for page in Page::<Size4KiB>::range_inclusive(start_page, end_page) {
                    // Translate the page and get the flags.
                    let res = self.page_table.translate(page.start_address());
                    let flags = match res {
                        TranslateResult::Mapped {
                            frame: _,
                            offset: _,
                            flags,
                        } => flags,
                        TranslateResult::NotMapped | TranslateResult::InvalidFrameAddress(_) => {
                            unreachable!("has the elf file not been mapped correctly?")
                        }
                    };

                    if flags.contains(COPIED) {
                        unsafe {
                            self.page_table
                                .update_flags(page, flags & !COPIED)
                                .unwrap()
                                .ignore();
                        }
                    }
                }
            }
        }
        Ok(())
    }

    fn handle_tls_segment(&mut self, segment: ProgramHeader) -> Result<TlsTemplate, &'static str> {
        Ok(TlsTemplate {
            start_address: self.virtual_address_offset + segment.virtual_addr(),
            mem_size: segment.mem_size(),
            file_size: segment.file_size(),
        })
    }

    fn handle_dynamic_segment(
        &mut self,
        segment: ProgramHeader,
        elf_file: &ElfFile,
    ) -> Result<(), &'static str> {
        let data = segment.get_data(elf_file)?;
        let data = if let SegmentData::Dynamic64(data) = data {
            data
        } else {
            panic!("expected Dynamic64 segment")
        };

        // Find the `Rela`, `RelaSize` and `RelaEnt` entries.
        let mut rela = None;
        let mut rela_size = None;
        let mut rela_ent = None;
        for rel in data {
            let tag = rel.get_tag()?;
            match tag {
                dynamic::Tag::Rela => {
                    let ptr = rel.get_ptr()?;
                    let prev = rela.replace(ptr);
                    if prev.is_some() {
                        return Err("Dynamic section contains more than one Rela entry");
                    }
                }
                dynamic::Tag::RelaSize => {
                    let val = rel.get_val()?;
                    let prev = rela_size.replace(val);
                    if prev.is_some() {
                        return Err("Dynamic section contains more than one RelaSize entry");
                    }
                }
                dynamic::Tag::RelaEnt => {
                    let val = rel.get_val()?;
                    let prev = rela_ent.replace(val);
                    if prev.is_some() {
                        return Err("Dynamic section contains more than one RelaEnt entry");
                    }
                }
                _ => {}
            }
        }
        let offset = if let Some(rela) = rela {
            rela
        } else {
            if rela_size.is_some() || rela_ent.is_some() {
                return Err("Rela entry is missing but RelaSize or RelaEnt have been provided");
            }

            return Ok(());
        };
        let total_size = rela_size.ok_or("RelaSize entry is missing")?;
        let entry_size = rela_ent.ok_or("RelaEnt entry is missing")?;

        assert_eq!(
            entry_size,
            size_of::<Rela<u64>>() as u64,
            "unsupported entry size: {entry_size}"
        );

        let num_entries = total_size / entry_size;
        for idx in 0..num_entries {
            let rela = self.read_relocation(offset, idx);
            self.apply_relocation(rela, elf_file)?;
        }

        Ok(())
    }

    fn read_relocation(&self, relocation_table: u64, idx: u64) -> Rela<u64> {
        let offset = relocation_table + size_of::<Rela<u64>>() as u64 * idx;
        let value = self.virtual_address_offset + offset;
        let addr = VirtAddr::try_new(value).expect("relocation table is outside the address space");

        let mut buf = [0; 24];
        self.copy_from(addr, &mut buf);

        unsafe {
            core::ptr::read_unaligned(&buf as *const u8 as *const Rela<u64>)
        }
    }

    fn apply_relocation(
        &mut self,
        rela: Rela<u64>,
        elf_file: &ElfFile,
    ) -> Result<(), &'static str> {
        let symbol_idx = rela.get_symbol_table_index();
        assert_eq!(
            symbol_idx, 0,
            "relocations using the symbol table are not supported"
        );

        match rela.get_type() {
            8 => {
                check_is_in_load(elf_file, rela.get_offset())?;

                let addr = self.virtual_address_offset + rela.get_offset();
                let addr = VirtAddr::new(addr);

                let value = self.virtual_address_offset + rela.get_addend();

                unsafe {
                    self.copy_to(addr, &value.to_ne_bytes());
                }
            }
            ty => unimplemented!("relocation type {:x} not supported", ty),
        }

        Ok(())
    }

    fn handle_relro_segment(&mut self, program_header: ProgramHeader) {
        let start = self.virtual_address_offset + program_header.virtual_addr();
        let end = start + program_header.mem_size();
        let start = VirtAddr::new(start);
        let end = VirtAddr::new(end);
        let start_page = Page::containing_address(start);
        let end_page = Page::containing_address(end - 1u64);
        for page in Page::<Size4KiB>::range_inclusive(start_page, end_page) {
            // Translate the page and get the flags.
            let res = self.page_table.translate(page.start_address());
            let flags = match res {
                TranslateResult::Mapped {
                    frame: _,
                    offset: _,
                    flags,
                } => flags,
                TranslateResult::NotMapped | TranslateResult::InvalidFrameAddress(_) => {
                    unreachable!("has the elf file not been mapped correctly?")
                }
            };

            if flags.contains(Flags::WRITABLE) {
                unsafe {
                    self.page_table
                        .update_flags(page, flags & !Flags::WRITABLE)
                        .unwrap()
                        .ignore();
                }
            }
        }
    }
}

impl<'a, M, F> Loader<'a, M, F>
    where
        M: MapperAllSizes + Translate,
        F: FrameAllocator<Size4KiB>,
{
    fn new(
        kernel: Kernel<'a>,
        page_table: &'a mut M,
        frame_allocator: &'a mut F,
        used_entries: &mut Entries,
    ) -> Result<Self, &'static str> {
        let kernel_offset = PhysAddr::new(&kernel.elf.input[0] as *const u8 as u64);
        if !kernel_offset.is_aligned(PAGE_SIZE) {
            return Err("Loaded kernel ELF file is not sufficiently aligned");
        }

        let elf_file = kernel.elf;
        for program_header in elf_file.program_iter() {
            program::sanity_check(program_header, &elf_file)?;
        }

        let virtual_address_offset = match elf_file.header.pt2.type_().as_type() {
            header::Type::None => unimplemented!(),
            header::Type::Relocatable => unimplemented!(),
            header::Type::Executable => VirtualAddressOffset::zero(),
            header::Type::SharedObject => {
                let max_addr = elf_file
                    .program_iter()
                    .filter(|h| matches!(h.get_type(), Ok(Type::Load)))
                    .map(|h| h.virtual_addr() + h.mem_size())
                    .max()
                    .unwrap_or(0);
                let min_addr = elf_file
                    .program_iter()
                    .filter(|h| matches!(h.get_type(), Ok(Type::Load)))
                    .map(|h| h.virtual_addr())
                    .min()
                    .unwrap_or(0);

                let size = max_addr - min_addr;
                let align = elf_file
                    .program_iter()
                    .filter(|h| matches!(h.get_type(), Ok(Type::Load))).map(|h| h.align()).max().unwrap_or(1);

                let offset = used_entries.get_free_address(size, align).as_u64();
                VirtualAddressOffset::new(i128::from(offset) - i128::from(min_addr))
            }
            header::Type::Core => unimplemented!(),
            header::Type::ProcessorSpecific(_) => unimplemented!(),
        };

        used_entries.mark_segments(elf_file.program_iter(), virtual_address_offset);

        header::sanity_check(&elf_file)?;
        let loader = Loader {
            elf_file,
            inner: Inner {
                kernel_offset,
                virtual_address_offset,
                page_table,
                frame_allocator,
            },
        };

        Ok(loader)
    }

    fn load_segments(&mut self) -> Result<Option<TlsTemplate>, &'static str> {
        let mut tls_template = None;
        for program_header in self.elf_file.program_iter() {
            match program_header.get_type()? {
                Type::Load => self.inner.handle_load_segment(program_header)?,
                Type::Tls => {
                    if tls_template.is_none() {
                        tls_template = Some(self.inner.handle_tls_segment(program_header)?);
                    } else {
                        return Err("multiple TLS segments not supported");
                    }
                }
                Type::Null
                | Type::Dynamic
                | Type::Interp
                | Type::Note
                | Type::ShLib
                | Type::Phdr
                | Type::GnuRelro
                | Type::OsSpecific(_)
                | Type::ProcessorSpecific(_) => {}
            }
        }

        for program_header in self.elf_file.program_iter() {
            if let Type::Dynamic = program_header.get_type()? {
                self.inner
                    .handle_dynamic_segment(program_header, &self.elf_file)?
            }
        }

        for program_header in self.elf_file.program_iter() {
            if let Type::GnuRelro = program_header.get_type()? {
                self.inner.handle_relro_segment(program_header);
            }
        }

        self.inner.remove_copied_flags(&self.elf_file).unwrap();

        Ok(tls_template)
    }

    fn entry_point(&self) -> VirtAddr {
        VirtAddr::new(self.inner.virtual_address_offset + self.elf_file.header.pt2.entry_point())
    }
}

pub fn load_kernel(
    kernel: Kernel<'_>,
    page_table: &mut (impl MapperAllSizes + Translate),
    frame_allocator: &mut impl FrameAllocator<Size4KiB>,
    used_entries: &mut Entries,
) -> Result<(VirtAddr, Option<TlsTemplate>), &'static str> {
    let mut loader = Loader::new(kernel, page_table, frame_allocator, used_entries)?;
    let tls_template = loader.load_segments()?;

    Ok((loader.entry_point(), tls_template))
}
