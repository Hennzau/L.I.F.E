use x86_64::{PhysAddr, structures::paging::PageTable, VirtAddr};
use x86_64::structures::paging::{FrameAllocator, Mapper, OffsetPageTable, Page, PhysFrame, Size4KiB};
use synapse::memory::{MemoryRegionKind, MemoryRegions};

pub mod allocator;

pub struct NukleusFrameAllocator {
    memory_map: &'static MemoryRegions,
    next: usize,
}

impl NukleusFrameAllocator {
    pub unsafe fn init(memory_map: &'static MemoryRegions) -> Self {
        NukleusFrameAllocator {
            memory_map,
            next: 0,
        }
    }

    fn usable_frames(&self) -> impl Iterator<Item=PhysFrame> {
        let regions = self.memory_map.iter();
        let usable_regions = regions
            .filter(|r| r.kind == MemoryRegionKind::Usable);

        let addr_ranges = usable_regions
            .map(|r| r.start..r.end);

        let frame_addresses = addr_ranges.flat_map(|r| r.step_by(4096));

        frame_addresses.map(|addr| PhysFrame::containing_address(PhysAddr::new(addr)))
    }
}

unsafe impl FrameAllocator<Size4KiB> for NukleusFrameAllocator {
    fn allocate_frame(&mut self) -> Option<PhysFrame> {
        let frame = self.usable_frames().nth(self.next);
        self.next += 1;
        frame
    }
}

unsafe fn active_level_4_table(physical_memory_offset: VirtAddr)
                               -> &'static mut PageTable
{
    use x86_64::registers::control::Cr3;

    let (level_4_table_frame, _) = Cr3::read();

    let phys = level_4_table_frame.start_address();
    let virt = physical_memory_offset + phys.as_u64();
    let page_table_ptr: *mut PageTable = virt.as_mut_ptr();

    &mut *page_table_ptr
}

pub unsafe fn init(physical_memory_offset: VirtAddr) -> OffsetPageTable<'static> {
    let level_4_table = active_level_4_table(physical_memory_offset);
    OffsetPageTable::new(level_4_table, physical_memory_offset)
}

pub fn create_mapping(
    page: Page,
    physical_address: PhysAddr,
    mapper: &mut OffsetPageTable,
    frame_allocator: &mut impl FrameAllocator<Size4KiB>,
) {
    use x86_64::structures::paging::PageTableFlags as Flags;

    let frame = PhysFrame::containing_address(physical_address);
    let flags = Flags::PRESENT | Flags::WRITABLE;

    let map_to_result = unsafe {
        // FIXME: this is not safe, we do it only for testing
        mapper.map_to(page, frame, flags, frame_allocator)
    };

    map_to_result.expect("map_to failed").flush();
}