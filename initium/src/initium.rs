use core::alloc::Layout;
use core::arch::asm;
use core::mem::MaybeUninit;
use core::slice;
use usize_conversions::FromUsize;
use x86_64::{PhysAddr, structures::{
    paging::{
        PhysFrame,
        Page,
        Size4KiB,
        PageSize,
        OffsetPageTable,
    },
}, VirtAddr};
use x86_64::structures::paging::{FrameAllocator, Mapper, PageTableFlags, PageTableIndex, Size2MiB};
use x86_64::structures::paging::page_table::PageTableLevel;

use synapse::boot::{BootConfig, BootInfo};
use synapse::framebuffer::{Framebuffer, FramebufferInfo};
use synapse::memory::MemoryRegion;
use synapse::tls_template::TlsTemplate;
use crate::entries::Entries;
use crate::gdt::create_and_load;
use crate::kernel;
use crate::kernel::{Kernel, load_kernel};
use crate::memory::{LegacyFrameAllocator, LegacyMemoryRegion};

#[derive(Debug, Copy, Clone)]
pub struct RawFramebufferInfo {
    pub addr: PhysAddr,
    pub info: FramebufferInfo,
}

#[derive(Debug, Copy, Clone)]
pub struct SystemInfo {
    pub framebuffer: Option<RawFramebufferInfo>,

    pub rsdp_addr: Option<PhysAddr>,
    pub ramdisk_addr: Option<u64>,
    pub ramdisk_len: u64,
}

fn enable_nxe_bit() {
    use x86_64::registers::control::{Efer, EferFlags};
    unsafe { Efer::update(|efer| *efer |= EferFlags::NO_EXECUTE_ENABLE) }
}

fn enable_write_protect_bit() {
    use x86_64::registers::control::{Cr0, Cr0Flags};
    unsafe { Cr0::update(|cr0| *cr0 |= Cr0Flags::WRITE_PROTECT) };
}

struct Addresses {
    page_table: PhysFrame,
    stack_top: VirtAddr,
    entry_point: VirtAddr,
    boot_info: &'static mut BootInfo,
}

unsafe fn context_switch(addresses: Addresses) -> ! {
    unsafe {
        asm!(
        "mov cr3, {}; mov rsp, {}; push 0; jmp {}",
        in(reg) addresses.page_table.start_address().as_u64(),
        in(reg) addresses.stack_top.as_u64(),
        in(reg) addresses.entry_point.as_u64(),
        in("rdi") addresses.boot_info as *const _ as usize,
        );
    }
    unreachable!();
}

fn mapping_addr_page_aligned(
    size: u64,
    used_entries: &mut Entries,
    kind: &str,
) -> Page {
    match mapping_addr(size, Size4KiB::SIZE, used_entries) {
        Ok(addr) => Page::from_start_address(addr).unwrap(),
        Err(addr) => panic!("{kind} address must be page-aligned (is `{addr:?})`"),
    }
}

fn mapping_addr(
    size: u64,
    alignment: u64,
    used_entries: &mut Entries,
) -> Result<VirtAddr, VirtAddr> {
    let addr = used_entries.get_free_address(size, alignment);

    if addr.is_aligned(alignment) {
        Ok(addr)
    } else {
        Err(addr)
    }
}

pub struct PageTables {
    pub bootloader: OffsetPageTable<'static>,
    pub kernel: OffsetPageTable<'static>,
    pub kernel_level_4_frame: PhysFrame,
}

pub struct Mappings {
    pub entry_point: VirtAddr,
    pub stack_top: VirtAddr,
    pub used_entries: Entries,
    pub framebuffer: Option<VirtAddr>,
    /// The start address of the physical memory mapping, if enabled.
    pub physical_memory_offset: Option<VirtAddr>,
    /// The level 4 page table index of the recursive mapping, if enabled.
    pub recursive_index: Option<PageTableIndex>,
    /// The thread local storage template of the kernel executable, if it contains one.
    pub tls_template: Option<TlsTemplate>,

    pub kernel_slice_start: u64,
    pub kernel_slice_len: u64,
    pub ramdisk_slice_start: Option<VirtAddr>,
    pub ramdisk_slice_len: u64,
}

pub fn set_up_mappings<I, D>(
    kernel: Kernel,
    frame_allocator: &mut LegacyFrameAllocator<I, D>,
    page_tables: &mut PageTables,
    framebuffer: Option<&RawFramebufferInfo>,
    system_info: &SystemInfo,
) -> Mappings
    where
        I: ExactSizeIterator<Item=D> + Clone,
        D: LegacyMemoryRegion,
{
    let kernel_page_table = &mut page_tables.kernel;

    let mut used_entries = Entries::new();

    enable_nxe_bit();
    enable_write_protect_bit();

    let kernel_slice_start = kernel.start_address as u64;
    let kernel_slice_len = u64::try_from(kernel.len).unwrap();

    let (entry_point, tls_template) = load_kernel(
        kernel,
        kernel_page_table,
        frame_allocator,
        &mut used_entries,
    )
        .expect("no entry point");

    let kernel_stack_size = 80 * 1024;

    let stack_start = {
        let guard_page = mapping_addr_page_aligned(
            Size4KiB::SIZE + kernel_stack_size,
            &mut used_entries,
            "kernel stack start",
        );
        guard_page + 1
    };

    let stack_end_addr = stack_start.start_address() + kernel_stack_size;

    let stack_end = Page::containing_address(stack_end_addr - 1u64);
    for page in Page::range_inclusive(stack_start, stack_end) {
        let frame = frame_allocator
            .allocate_frame()
            .expect("frame allocation failed when mapping a kernel stack");
        let flags = PageTableFlags::PRESENT | PageTableFlags::WRITABLE;
        match unsafe { kernel_page_table.map_to(page, frame, flags, frame_allocator) } {
            Ok(tlb) => tlb.flush(),
            Err(err) => panic!("failed to map page {:?}: {:?}", page, err),
        }
    }

    let context_switch_function = PhysAddr::new(context_switch as *const () as u64);
    let context_switch_function_start_frame: PhysFrame =
        PhysFrame::containing_address(context_switch_function);
    for frame in PhysFrame::range_inclusive(
        context_switch_function_start_frame,
        context_switch_function_start_frame + 1,
    ) {
        match unsafe {
            kernel_page_table.identity_map(frame, PageTableFlags::PRESENT, frame_allocator)
        } {
            Ok(tlb) => tlb.flush(),
            Err(err) => panic!("failed to identity map frame {:?}: {:?}", frame, err),
        }
    }

    let gdt_frame = frame_allocator
        .allocate_frame()
        .expect("failed to allocate GDT frame");
    create_and_load(gdt_frame);

    match unsafe {
        kernel_page_table.identity_map(gdt_frame, PageTableFlags::PRESENT, frame_allocator)
    } {
        Ok(tlb) => tlb.flush(),
        Err(err) => panic!("failed to identity map frame {:?}: {:?}", gdt_frame, err),
    }

    let framebuffer_virt_addr = if let Some(framebuffer) = framebuffer {
        let framebuffer_start_frame: PhysFrame = PhysFrame::containing_address(framebuffer.addr);
        let framebuffer_end_frame =
            PhysFrame::containing_address(framebuffer.addr + framebuffer.info.byte_len - 1u64);
        let start_page = mapping_addr_page_aligned(
            u64::from_usize(framebuffer.info.byte_len),
            &mut used_entries,
            "framebuffer",
        );
        for (i, frame) in
        PhysFrame::range_inclusive(framebuffer_start_frame, framebuffer_end_frame).enumerate()
        {
            let page = start_page + u64::from_usize(i);
            let flags = PageTableFlags::PRESENT | PageTableFlags::WRITABLE;
            match unsafe { kernel_page_table.map_to(page, frame, flags, frame_allocator) } {
                Ok(tlb) => tlb.flush(),
                Err(err) => panic!(
                    "failed to map page {:?} to frame {:?}: {:?}",
                    page, frame, err
                ),
            }
        }
        let framebuffer_virt_addr = start_page.start_address();
        Some(framebuffer_virt_addr)
    } else {
        None
    };
    let ramdisk_slice_len = system_info.ramdisk_len;
    let ramdisk_slice_start = if let Some(ramdisk_address) = system_info.ramdisk_addr {
        let start_page = mapping_addr_page_aligned(
            system_info.ramdisk_len,
            &mut used_entries,
            "ramdisk start",
        );
        let physical_address = PhysAddr::new(ramdisk_address);
        let ramdisk_physical_start_page: PhysFrame<Size4KiB> =
            PhysFrame::containing_address(physical_address);
        let ramdisk_page_count = (system_info.ramdisk_len - 1) / Size4KiB::SIZE;
        let ramdisk_physical_end_page = ramdisk_physical_start_page + ramdisk_page_count;

        let flags = PageTableFlags::PRESENT | PageTableFlags::WRITABLE;
        for (i, frame) in
        PhysFrame::range_inclusive(ramdisk_physical_start_page, ramdisk_physical_end_page)
            .enumerate()
        {
            let page = start_page + i as u64;
            match unsafe { kernel_page_table.map_to(page, frame, flags, frame_allocator) } {
                Ok(tlb) => tlb.ignore(),
                Err(err) => panic!(
                    "Failed to map page {:?} to frame {:?}: {:?}",
                    page, frame, err
                ),
            };
        }
        Some(start_page.start_address())
    } else {
        None
    };

    Mappings {
        framebuffer: framebuffer_virt_addr,
        entry_point,
        stack_top: stack_end_addr.align_down(16u8),
        used_entries,
        physical_memory_offset: Option::None,
        recursive_index: Option::None,
        tls_template,

        kernel_slice_start,
        kernel_slice_len,
        ramdisk_slice_start,
        ramdisk_slice_len,
    }
}

pub fn create_boot_info<I, D>(
    boot_config: &BootConfig,
    mut frame_allocator: LegacyFrameAllocator<I, D>,
    page_tables: &mut PageTables,
    mappings: &mut Mappings,
    system_info: SystemInfo,
) -> &'static mut BootInfo
    where
        I: ExactSizeIterator<Item=D> + Clone,
        D: LegacyMemoryRegion,
{
    let (boot_info, memory_regions) = {
        let boot_info_layout = Layout::new::<BootInfo>();
        let regions = frame_allocator.len() + 4; // up to 4 regions might be split into used/unused
        let memory_regions_layout = Layout::array::<MemoryRegion>(regions).unwrap();
        let (combined, memory_regions_offset) =
            boot_info_layout.extend(memory_regions_layout).unwrap();

        let boot_info_addr = mapping_addr(
            u64::from_usize(combined.size()),
            u64::from_usize(combined.align()),
            &mut mappings.used_entries,
        )
            .expect("boot info addr is not properly aligned");

        let memory_map_regions_addr = boot_info_addr + memory_regions_offset;
        let memory_map_regions_end = boot_info_addr + combined.size();

        let start_page = Page::containing_address(boot_info_addr);
        let end_page = Page::containing_address(memory_map_regions_end - 1u64);
        for page in Page::range_inclusive(start_page, end_page) {
            let flags = PageTableFlags::PRESENT | PageTableFlags::WRITABLE;
            let frame = frame_allocator
                .allocate_frame()
                .expect("frame allocation for boot info failed");
            match unsafe {
                page_tables
                    .kernel
                    .map_to(page, frame, flags, &mut frame_allocator)
            } {
                Ok(tlb) => tlb.flush(),
                Err(err) => panic!("failed to map page {:?}: {:?}", page, err),
            }

            match unsafe {
                page_tables
                    .bootloader
                    .map_to(page, frame, flags, &mut frame_allocator)
            } {
                Ok(tlb) => tlb.flush(),
                Err(err) => panic!("failed to map page {:?}: {:?}", page, err),
            }
        }

        let boot_info: &'static mut MaybeUninit<BootInfo> =
            unsafe { &mut *boot_info_addr.as_mut_ptr() };
        let memory_regions: &'static mut [MaybeUninit<MemoryRegion>] =
            unsafe { slice::from_raw_parts_mut(memory_map_regions_addr.as_mut_ptr(), regions) };
        (boot_info, memory_regions)
    };

    let memory_regions = frame_allocator.construct_memory_map(
        memory_regions,
        mappings.kernel_slice_start,
        mappings.kernel_slice_len,
    );

    let boot_info = boot_info.write({
        let mut info = BootInfo::new(memory_regions.into());
        info.framebuffer = mappings
            .framebuffer
            .map(|addr| unsafe {
                Framebuffer::new(
                    addr.as_u64(),
                    system_info
                        .framebuffer
                        .expect(
                            "there shouldn't be a mapping for the framebuffer if there is \
                            no framebuffer",
                        )
                        .info,
                )
            })
            .into();
        info.physical_memory_offset = mappings.physical_memory_offset.map(VirtAddr::as_u64).into();
        info.recursive_index = mappings.recursive_index.map(Into::into).into();
        info.rsdp_address = system_info.rsdp_addr.map(|addr| addr.as_u64()).into();
        info.tls_template = mappings.tls_template.into();
        info.ramdisk_address = mappings
            .ramdisk_slice_start
            .map(|addr| addr.as_u64())
            .into();
        info.ramdisk_len = mappings.ramdisk_slice_len;
        info
    });

    boot_info
}

pub fn switch_to_kernel(
    page_tables: PageTables,
    mappings: Mappings,
    boot_info: &'static mut BootInfo,
) -> ! {
    let PageTables {
        kernel_level_4_frame,
        ..
    } = page_tables;
    let addresses = Addresses {
        page_table: kernel_level_4_frame,
        stack_top: mappings.stack_top,
        entry_point: mappings.entry_point,
        boot_info,
    };

    unsafe {
        context_switch(addresses);
    }
}

pub fn load_and_switch_to_kernel<I, D>(
    kernel: Kernel,
    boot_config: BootConfig,
    mut frame_allocator: LegacyFrameAllocator<I, D>,
    mut page_tables: PageTables,
    system_info: SystemInfo,
) -> !
    where
        I: ExactSizeIterator<Item=D> + Clone,
        D: LegacyMemoryRegion,
{
    let mut mappings = set_up_mappings(
        kernel,
        &mut frame_allocator,
        &mut page_tables,
        system_info.framebuffer.as_ref(),
        &system_info,
    );

    let boot_info = create_boot_info(
        &boot_config,
        frame_allocator,
        &mut page_tables,
        &mut mappings,
        system_info,
    );

    switch_to_kernel(page_tables, mappings, boot_info);
}