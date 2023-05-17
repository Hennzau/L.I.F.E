#![no_std]
#![no_main]

extern crate alloc;

mod memory;

use alloc::boxed::Box;
use ab_glyph::{Font, FontRef, Glyph, OutlineCurve, point};
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

    writer.draw_line(buffer, (info.width / 2, info.height / 2), (info.width / 2 + 300, info.height / 2 + 100), 3, Color {
        red: 255,
        green: 0,
        blue: 0,
    });

    writer.draw_line(buffer, (info.width / 2, info.height / 2), (info.width / 2 + 100, info.height / 2 + 300), 3, Color {
        red: 255,
        green: 0,
        blue: 0,
    });

    writer.draw_line(buffer, (info.width / 2, info.height / 2), (info.width / 2 + 300, info.height / 2 - 100), 3, Color {
        red: 255,
        green: 0,
        blue: 0,
    });

    writer.draw_line(buffer, (info.width / 2, info.height / 2), (info.width / 2 + 100, info.height / 2 - 300), 3, Color {
        red: 255,
        green: 0,
        blue: 0,
    });

    writer.draw_line(buffer, (info.width / 2, info.height / 2), (info.width / 2 - 300, info.height / 2 + 100), 3, Color {
        red: 255,
        green: 0,
        blue: 0,
    });

    writer.draw_line(buffer, (info.width / 2, info.height / 2), (info.width / 2 - 100, info.height / 2 + 300), 3, Color {
        red: 255,
        green: 0,
        blue: 0,
    });

    writer.draw_line(buffer, (info.width / 2, info.height / 2), (info.width / 2 - 300, info.height / 2 - 100), 3, Color {
        red: 255,
        green: 0,
        blue: 0,
    });

    writer.draw_line(buffer, (info.width / 2, info.height / 2), (info.width / 2 - 100, info.height / 2 - 300), 3, Color {
        red: 255,
        green: 0,
        blue: 0,
    });

    writer.draw_disk(buffer, info.width / 2, info.height / 2, 25, Color {
        red: 0,
        green: 0,
        blue: 255,
    });

    writer.draw_line(buffer, (info.width / 2, info.height / 2), (info.width / 2, 0), 5, Color {
        red: 0,
        green: 0,
        blue: 0,
    });

    /* char */

    let font = FontRef::try_from_slice(include_bytes!("Montserrat-Regular.otf")).expect("Error constructing FontRef");
    let outline = font.outline(font.glyph_id('E')).unwrap();

    struct Line {
        x_0: f32,
        y_0: f32,
        x_1: f32,
        y_1: f32,

    }

    let lines = outline.curves.iter().filter_map(|curve| {
        match curve {
            OutlineCurve::Line(p0, p1) => Some(Line {
                x_0: p0.x,
                y_0: p0.y,
                x_1: p1.x,
                y_1: p1.y,
            }),
            _ => None,
        }
    });

    for line in lines {
        let x_0 = (line.x_0 * 1.0) as usize;
        let y_0 = (line.y_0 * 1.0) as usize;
        let x_1 = (line.x_1 * 1.0) as usize;
        let y_1 = (line.y_1 * 1.0) as usize;

        writer.draw_line(buffer, (x_0, y_0), (x_1, y_1), 5, Color {
            red: 0,
            green: 0,
            blue: 0,
        });
    }


    loop {}
}

synapse::entry_point!(main);