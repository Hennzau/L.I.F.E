#![no_std]
#![no_main]

use synapse::boot::BootInfo;
use synapse::optional::Optional;

#[panic_handler]
fn panic(_info: &core::panic::PanicInfo) -> ! {
    loop {}
}

fn main(boot_info: &'static mut BootInfo) -> ! {
    let framebuffer = core::mem::replace(&mut boot_info.framebuffer, Optional::None).into_option().unwrap();

    let info = framebuffer.info;
    let buffer = framebuffer.into_buffer();

    for j in 0..info.height {
        for i in 0..info.width {
            let index = (i + j * info.stride) * info.bytes_per_pixel;

            buffer[0 + index] = 255;
            buffer[1 + index] = 255;
            buffer[2 + index] = 255;
        }
    }

    loop {}
}

synapse::entry_point!(main);