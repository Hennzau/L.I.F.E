use core::{ops, slice};

#[derive(Debug, Copy, Clone, Eq, PartialEq)]
#[non_exhaustive]
#[repr(C)]
pub enum MemoryRegionKind {
    Usable,
    Bootloader,
    UnknownUefi(u32),
}

#[derive(Debug, Copy, Clone, Eq, PartialEq)]
#[repr(C)]
pub struct MemoryRegion {
    pub start: u64,
    pub end: u64,
    pub kind: MemoryRegionKind,
}

impl MemoryRegion {
    pub const fn empty() -> Self {
        MemoryRegion {
            start: 0,
            end: 0,
            kind: MemoryRegionKind::Bootloader,
        }
    }
}

#[derive(Debug)]
#[repr(C)]
pub struct MemoryRegions {
    pub ptr: *mut MemoryRegion,
    pub len: usize,
}

impl ops::Deref for MemoryRegions {
    type Target = [MemoryRegion];

    fn deref(&self) -> &Self::Target {
        unsafe { slice::from_raw_parts(self.ptr, self.len) }
    }
}

impl ops::DerefMut for MemoryRegions {
    fn deref_mut(&mut self) -> &mut Self::Target {
        unsafe { slice::from_raw_parts_mut(self.ptr, self.len) }
    }
}

impl From<&'static mut [MemoryRegion]> for MemoryRegions {
    fn from(regions: &'static mut [MemoryRegion]) -> Self {
        MemoryRegions {
            ptr: regions.as_mut_ptr(),
            len: regions.len(),
        }
    }
}

impl From<MemoryRegions> for &'static mut [MemoryRegion] {
    fn from(regions: MemoryRegions) -> &'static mut [MemoryRegion] {
        unsafe { slice::from_raw_parts_mut(regions.ptr, regions.len) }
    }
}
