#![no_std]

pub mod optional;
pub mod tls_template;
pub mod framebuffer;
pub mod memory;
pub mod boot;

#[macro_export]
macro_rules! entry_point {
    ($path:path) => {
        #[export_name = "_start"]
        pub extern "C" fn __impl_start(boot_info: &'static mut $crate::boot::BootInfo) -> ! {
            let f: fn(&'static mut $crate::boot::BootInfo) -> ! = $path;

            f(boot_info)
        }
    };
}