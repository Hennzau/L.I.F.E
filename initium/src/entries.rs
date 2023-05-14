use x86_64::{
    structures::paging::{Page, PageTableIndex},
    VirtAddr,
};

use usize_conversions::IntoUsize;
use x86_64::structures::paging::Size4KiB;
use xmas_elf::program::ProgramHeader;

use crate::kernel::VirtualAddressOffset;

pub struct Entries {
    entry_state: [bool; 512],
}

impl Entries {
    pub fn new() -> Self {
        let mut used = Entries {
            entry_state: [false; 512]
        };
        used.entry_state[0] = true;

        used
    }

    pub fn mark_segments<'a>(
        &mut self,
        segments: impl Iterator<Item=ProgramHeader<'a>>,
        virtual_address_offset: VirtualAddressOffset,
    ) {
        for segment in segments.filter(|s| s.mem_size() > 0) {
            self.mark_range_as_used(
                virtual_address_offset + segment.virtual_addr(),
                segment.mem_size(),
            );
        }
    }

    fn mark_range_as_used<S>(&mut self, address: u64, size: S)
        where
            VirtAddr: core::ops::Add<S, Output = VirtAddr>,
    {
        let start = VirtAddr::new(address);
        let end_inclusive = (start + size) - 1usize;
        let start_page = Page::<Size4KiB>::containing_address(start);
        let end_page_inclusive = Page::<Size4KiB>::containing_address(end_inclusive);

        for p4_index in u16::from(start_page.p4_index())..=u16::from(end_page_inclusive.p4_index())
        {
            self.mark_p4_index_as_used(PageTableIndex::new(p4_index));
        }
    }

    fn mark_p4_index_as_used(&mut self, p4_index: PageTableIndex) {
        self.entry_state[usize::from(p4_index)] = true;
    }

    pub fn get_free_entries(&mut self, num: u64) -> PageTableIndex {
        let mut free_entries = self
            .entry_state
            .windows(num.into_usize())
            .enumerate()
            .filter(|(_, entries)| entries.iter().all(|&used| !used))
            .map(|(idx, _)| idx);

        let idx_opt = free_entries.next();

        let Some(idx) = idx_opt else { panic!("no usable level 4 entries found ({num} entries requested)"); };

        // Mark the entries as used.
        for i in 0..num.into_usize() {
            self.entry_state[idx + i] = true;
        }

        PageTableIndex::new(idx.try_into().unwrap())
    }

    pub fn get_free_address(&mut self, size: u64, alignment: u64) -> VirtAddr {
        assert!(alignment.is_power_of_two());

        const LEVEL_4_SIZE: u64 = 4096 * 512 * 512 * 512;

        let level_4_entries = (size + (LEVEL_4_SIZE - 1)) / LEVEL_4_SIZE;

        Page::from_page_table_indices_1gib(
            self.get_free_entries(level_4_entries),
            PageTableIndex::new(0),
        ).start_address()
    }
}