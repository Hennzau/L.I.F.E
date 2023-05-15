use crate::optional::Optional;
use crate::framebuffer::Framebuffer;
use crate::memory::MemoryRegions;
use crate::tls_template::TlsTemplate;

pub struct BootConfig {
    pub framebuffer_width: usize,
    pub framebuffer_height: usize,
}

pub struct BootInfo {
    pub memory_regions: MemoryRegions,
    pub framebuffer: Optional<Framebuffer>,
    pub physical_memory_offset: Optional<u64>,
    pub rsdp_address: Optional<u64>,
    pub tls_template: Optional<TlsTemplate>,
    pub ramdisk_address: Optional<u64>,
    pub ramdisk_len: u64,
}

impl BootInfo {
    pub fn new(memory_regions: MemoryRegions) -> Self {
        Self {
            memory_regions,
            framebuffer: Optional::None,
            physical_memory_offset: Optional::None,
            rsdp_address: Optional::None,
            tls_template: Optional::None,
            ramdisk_address: Optional::None,
            ramdisk_len: 0,
        }
    }
}

