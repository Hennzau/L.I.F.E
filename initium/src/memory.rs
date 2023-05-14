use synapse::memory::{MemoryRegion, MemoryRegionKind};
use core::mem::MaybeUninit;
use x86_64::{
    structures::paging::{FrameAllocator, PhysFrame, Size4KiB},
    PhysAddr,
};

pub trait LegacyMemoryRegion: Copy {
    fn start(&self) -> PhysAddr;

    fn len(&self) -> u64;

    fn is_empty(&self) -> bool {
        self.len() == 0
    }

    fn kind(&self) -> MemoryRegionKind;

    fn usable_after_bootloader_exit(&self) -> bool;
}

pub struct LegacyFrameAllocator<I, D> {
    original: I,
    memory_map: I,
    current_descriptor: Option<D>,
    next_frame: PhysFrame,
}

impl<I, D> LegacyFrameAllocator<I, D>
    where
        I: ExactSizeIterator<Item=D> + Clone,
        I::Item: LegacyMemoryRegion,
{
    pub fn new(memory_map: I) -> Self {
        // skip frame 0 because the rust core library does not see 0 as a valid address
        let start_frame = PhysFrame::containing_address(PhysAddr::new(0x1000));

        Self::new_starting_at(start_frame, memory_map)
    }

    pub fn new_starting_at(frame: PhysFrame, memory_map: I) -> Self {
        Self {
            original: memory_map.clone(),
            memory_map,
            current_descriptor: None,
            next_frame: frame,
        }
    }

    fn allocate_frame_from_descriptor(&mut self, descriptor: D) -> Option<PhysFrame> {
        let start_addr = descriptor.start();
        let start_frame = PhysFrame::containing_address(start_addr);

        let end_addr = start_addr + descriptor.len();
        let end_frame = PhysFrame::containing_address(end_addr - 1u64);

        if self.next_frame < start_frame {
            self.next_frame = start_frame;
        }

        if self.next_frame <= end_frame {
            let ret = self.next_frame;
            self.next_frame += 1;
            Some(ret)
        } else {
            None
        }
    }

    /// Returns the number of memory regions in the underlying memory map.
    ///
    /// The function always returns the same value, i.e. the length doesn't
    /// change after calls to `allocate_frame`.
    pub fn len(&self) -> usize {
        self.original.len()
    }

    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    pub fn max_physical_address(&self) -> PhysAddr {
        self.original
            .clone()
            .map(|r| r.start() + r.len())
            .max()
            .unwrap()
    }

    fn add_region(
        region: MemoryRegion,
        regions: &mut [MaybeUninit<MemoryRegion>],
        next_index: &mut usize,
    ) {
        if region.start == region.end {
            return;
        }
        unsafe {
            regions
                .get_mut(*next_index)
                .expect("cannot add region: no more free entries in memory map")
                .as_mut_ptr()
                .write(region)
        };
        *next_index += 1;
    }

    pub fn construct_memory_map(
        self,
        regions: &mut [MaybeUninit<MemoryRegion>],
        kernel_slice_start: u64,
        kernel_slice_len: u64,
    ) -> &mut [MemoryRegion] {
        let mut next_index = 0;

        for descriptor in self.original {
            let mut start = descriptor.start();
            let end = start + descriptor.len();
            let next_free = self.next_frame.start_address();

            let kind = match descriptor.kind() {
                MemoryRegionKind::Usable => {
                    if end <= next_free {
                        MemoryRegionKind::Bootloader
                    } else if descriptor.start() >= next_free {
                        MemoryRegionKind::Usable
                    } else {
                        let used_region = MemoryRegion {
                            start: descriptor.start().as_u64(),
                            end: next_free.as_u64(),
                            kind: MemoryRegionKind::Bootloader,
                        };

                        Self::add_region(used_region, regions, &mut next_index);

                        start = next_free;
                        MemoryRegionKind::Usable
                    }
                }
                _ if descriptor.usable_after_bootloader_exit() => {
                    MemoryRegionKind::Usable
                }

                other => other,
            };

            let region = MemoryRegion {
                start: start.as_u64(),
                end: end.as_u64(),
                kind,
            };

            let kernel_slice_end = kernel_slice_start + kernel_slice_len;
            if region.kind == MemoryRegionKind::Usable
                && kernel_slice_start < region.end
                && kernel_slice_end > region.start
            {
                // region overlaps with kernel -> we might need to split it

                assert!(
                    kernel_slice_start >= region.start,
                    "region overlaps with kernel, but kernel begins before region \
                    (kernel_slice_start: {kernel_slice_start:#x}, region_start: {:#x})",
                    region.start
                );

                assert!(
                    kernel_slice_end <= region.end,
                    "region overlaps with kernel, but region ends before kernel \
                    (kernel_slice_end: {kernel_slice_end:#x}, region_end: {:#x})",
                    region.end,
                );

                let before_kernel = MemoryRegion {
                    end: kernel_slice_start,
                    ..region
                };
                let kernel = MemoryRegion {
                    start: kernel_slice_start,
                    end: kernel_slice_end,
                    kind: MemoryRegionKind::Bootloader,
                };
                let after_kernel = MemoryRegion {
                    start: kernel_slice_end,
                    ..region
                };

                Self::add_region(before_kernel, regions, &mut next_index);
                Self::add_region(kernel, regions, &mut next_index);
                Self::add_region(after_kernel, regions, &mut next_index);
            } else {
                Self::add_region(region, regions, &mut next_index);
            }
        }

        let initialized = &mut regions[..next_index];
        unsafe {
            &mut *(initialized as *mut [_] as *mut [_])
        }
    }
}

unsafe impl<I, D> FrameAllocator<Size4KiB> for LegacyFrameAllocator<I, D>
    where
        I: ExactSizeIterator<Item=D> + Clone,
        I::Item: LegacyMemoryRegion,
{
    fn allocate_frame(&mut self) -> Option<PhysFrame<Size4KiB>> {
        if let Some(current_descriptor) = self.current_descriptor {
            match self.allocate_frame_from_descriptor(current_descriptor) {
                Some(frame) => return Some(frame),
                None => {
                    self.current_descriptor = None;
                }
            }
        }

        while let Some(descriptor) = self.memory_map.next() {
            if descriptor.kind() != MemoryRegionKind::Usable {
                continue;
            }

            if let Some(frame) = self.allocate_frame_from_descriptor(descriptor) {
                self.current_descriptor = Some(descriptor);
                return Some(frame);
            }
        }

        None
    }
}