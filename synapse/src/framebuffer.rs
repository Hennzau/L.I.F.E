pub mod writer;

use core::slice;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(C)]
pub enum PixelFormat {
    Rgb,
    Bgr,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Color {
    pub red: u8,
    pub green: u8,
    pub blue: u8,
}

#[derive(Debug, Clone, Copy)]
#[repr(C)]
pub struct FramebufferInfo {
    pub width: usize,
    pub height: usize,

    pub pixel_format: PixelFormat,

    pub byte_len: usize,
    pub bytes_per_pixel: usize,
    pub stride: usize,
}

#[repr(C)]
pub struct Framebuffer {
    pub start_address: u64,
    pub info: FramebufferInfo,
}

impl Framebuffer {
    pub unsafe fn new(start_address: u64, info: FramebufferInfo) -> Self {
        Self { start_address, info }
    }

    pub fn buffer(&self) -> &[u8] {
        unsafe { self.create_buffer() }
    }

    pub fn buffer_mut(&mut self) -> &mut [u8] {
        unsafe { self.create_buffer_mut() }
    }

    pub fn into_buffer(self) -> &'static mut [u8] {
        unsafe { self.create_buffer_mut() }
    }

    unsafe fn create_buffer<'a>(&self) -> &'a [u8] {
        unsafe { slice::from_raw_parts(self.start_address as *const u8, self.info.byte_len) }
    }

    unsafe fn create_buffer_mut<'a>(&self) -> &'a mut [u8] {
        unsafe { slice::from_raw_parts_mut(self.start_address as *mut u8, self.info.byte_len) }
    }

    pub fn info(&self) -> FramebufferInfo {
        self.info
    }
}
