#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(C)]
pub struct TlsTemplate {
    pub start_address: u64,
    pub file_size: u64,
    pub mem_size: u64,
}