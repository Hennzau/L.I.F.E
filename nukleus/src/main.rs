#![no_std]
#![no_main]

use synapse::boot::BootInfo;

#[panic_handler]
fn panic(_info: &core::panic::PanicInfo) -> ! {
    loop {}
}

fn main(boot_info: &'static mut BootInfo) -> ! {
    loop {}
}

synapse::entry_point!(main);