use crate::memory::LegacyMemoryRegion;

use synapse::memory::MemoryRegionKind;

use uefi::table::boot::{MemoryDescriptor, MemoryType};
use x86_64::PhysAddr;

#[derive(Copy, Clone)]
pub struct UefiMemoryDescriptor(pub MemoryDescriptor);

const PAGE_SIZE: u64 = 4096;

impl LegacyMemoryRegion for UefiMemoryDescriptor {
    fn start(&self) -> PhysAddr {
        PhysAddr::new(self.0.phys_start)
    }

    fn len(&self) -> u64 {
        self.0.page_count * PAGE_SIZE
    }

    fn kind(&self) -> MemoryRegionKind {
        match self.0.ty {
            MemoryType::CONVENTIONAL => MemoryRegionKind::Usable,
            other => MemoryRegionKind::UnknownUefi(other.0),
        }
    }

    fn usable_after_bootloader_exit(&self) -> bool {
        match self.0.ty {
            MemoryType::CONVENTIONAL => true,
            MemoryType::LOADER_CODE
            | MemoryType::LOADER_DATA
            | MemoryType::BOOT_SERVICES_CODE
            | MemoryType::BOOT_SERVICES_DATA => {
                true
            }
            MemoryType::RUNTIME_SERVICES_CODE | MemoryType::RUNTIME_SERVICES_DATA => {
                false
            }
            _ => false,
        }
    }
}