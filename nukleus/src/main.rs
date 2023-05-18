#![no_std]
#![no_main]
#![feature(core_intrinsics)]

extern crate alloc;

mod memory;
mod text_based_interface;

use x86_64::VirtAddr;

use synapse::boot::BootInfo;
use synapse::optional::Optional;
use synapse::framebuffer::Color;

use crate::memory::NukleusFrameAllocator;

use crate::text_based_interface::framebuffer_writer::FramebufferWriter;
use crate::text_based_interface::primitive::{Point, Primitive};

#[panic_handler]
fn panic(_info: &core::panic::PanicInfo) -> ! {
    loop {}
}

fn main(boot_info: &'static mut BootInfo) -> ! {
    /* retrieve data from BootInfo */

    let physical_memory_offset = VirtAddr::new(core::mem::replace(&mut boot_info.physical_memory_offset, Optional::None).into_option().unwrap());
    let framebuffer = core::mem::replace(&mut boot_info.framebuffer, Optional::None).into_option().unwrap();

    /* Manage the memory for the Kernel */

    let mut mapper = unsafe { memory::init(physical_memory_offset) };
    let mut frame_allocator = unsafe { NukleusFrameAllocator::init(&boot_info.memory_regions) };

    memory::allocator::init_heap(&mut mapper, &mut frame_allocator).expect("");

    /* Write to Framebuffer */

    let info = framebuffer.info;
    let buffer = framebuffer.into_buffer();
    let writer = FramebufferWriter::new(info);

    text_based_interface::draw_background(buffer, &writer);

    let quad = Primitive::Quad(Point { x: info.width / 2 - 100, y: info.height / 2 - 50 }, Point { x: info.width / 2 + 100, y: info.height / 2 + 50 });
    writer.draw_primitive(buffer, quad, Color {
        red: 255,
        green: 255,
        blue: 0,
    });

    let circle = Primitive::Circle(Point {
        x: info.width / 2,
        y: info.height / 2,
    }, 100);
    writer.draw_primitive(buffer, circle, Color {
        red: 0,
        green: 0,
        blue: 255,
    });

    let line = Primitive::Line(Point { x: info.width / 2, y: info.height / 2 }, Point { x: info.width / 2 - 100, y: info.height / 2 + 300 });
    writer.draw_primitive(buffer, line, Color {
        red: 0,
        green: 0,
        blue: 255,
    });

    let disk = Primitive::Disk(Point {
        x: info.width / 2,
        y: info.height / 2,
    }, 15);
    writer.draw_primitive(buffer, disk, Color {
        red: 255,
        green: 200,
        blue: 0,
    });

    loop {}
}

synapse::entry_point!(main);