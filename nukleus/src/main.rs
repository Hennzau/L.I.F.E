#![no_std]
#![no_main]

extern crate alloc;

mod memory;

use alloc::boxed::Box;
use x86_64::VirtAddr;
use synapse::boot::BootInfo;
use synapse::optional::Optional;
use synapse::framebuffer::{Color, writer::FramebufferWriter};

use crate::memory::NukleusFrameAllocator;

#[panic_handler]
fn panic(_info: &core::panic::PanicInfo) -> ! {
    loop {}
}

fn main(boot_info: &'static mut BootInfo) -> ! {
    let physical_memory_offset = VirtAddr::new(core::mem::replace(&mut boot_info.physical_memory_offset, Optional::None).into_option().unwrap());
    let mut mapper = unsafe { memory::init(physical_memory_offset) };
    let mut frame_allocator = unsafe { NukleusFrameAllocator::init(&boot_info.memory_regions) };

    memory::allocator::init_heap(&mut mapper, &mut frame_allocator).expect("TODO: panic message");

    let a = Box::new(9);

    let framebuffer = core::mem::replace(&mut boot_info.framebuffer, Optional::None).into_option().unwrap();

    let info = framebuffer.info;
    let buffer = framebuffer.into_buffer();
    let writer = FramebufferWriter::new(info);

    /* white background */

    writer.draw_quad(buffer, 0, 0, info.width, info.height, Color {
        red: 221,
        green: 232,
        blue: 242,
    });

    /* box */

    writer.draw_centered_quad(buffer, info.width / 2, info.height / 2, 304, 204, Color {
        red: 0,
        green: 0,
        blue: 0,
    });

    writer.draw_centered_quad(buffer, info.width / 2, info.height / 2, 300, 200, Color {
        red: 0,
        green: 255,
        blue: 0,
    });

    /* line */

    writer.draw_line(buffer, (info.width / 2, info.height / 2), (info.width / 2 + 300, info.height / 2 + 100), Color {
        red: 255,
        green: 0,
        blue: 0,
    });

    writer.draw_line(buffer, (info.width / 2, info.height / 2), (info.width / 2 + 100, info.height / 2 + 300), Color {
        red: 255,
        green: 0,
        blue: 0,
    });

    writer.draw_line(buffer, (info.width / 2, info.height / 2), (info.width / 2 + 300, info.height / 2 - 100), Color {
        red: 255,
        green: 0,
        blue: 0,
    });

    writer.draw_line(buffer, (info.width / 2, info.height / 2), (info.width / 2 + 100, info.height / 2 - 300), Color {
        red: 255,
        green: 0,
        blue: 0,
    });

    writer.draw_line(buffer, (info.width / 2, info.height / 2), (info.width / 2 - 300, info.height / 2 + 100), Color {
        red: 255,
        green: 0,
        blue: 0,
    });

    writer.draw_line(buffer, (info.width / 2, info.height / 2), (info.width / 2 - 100, info.height / 2 + 300), Color {
        red: 255,
        green: 0,
        blue: 0,
    });

    writer.draw_line(buffer, (info.width / 2, info.height / 2), (info.width / 2 - 300, info.height / 2 - 100), Color {
        red: 255,
        green: 0,
        blue: 0,
    });

    writer.draw_line(buffer, (info.width / 2, info.height / 2), (info.width / 2 - 100, info.height / 2 - 300), Color {
        red: 255,
        green: 0,
        blue: 0,
    });


    /* char */

    loop {}
}

synapse::entry_point!(main);