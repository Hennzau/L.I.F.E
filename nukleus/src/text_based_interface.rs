use synapse::framebuffer::Color;
use crate::text_based_interface::framebuffer_writer::FramebufferWriter;
use crate::text_based_interface::primitive::{Point, Primitive};

pub mod primitive;
pub mod framebuffer_writer;

pub fn draw_background(buffer: &mut [u8], writer: &FramebufferWriter) {
    let info = writer.info;

    let quad = Primitive::Quad(Point { x: 0, y: 0 }, Point { x: info.width, y: info.height });
    writer.draw_primitive(buffer, quad, Color {
        red: 221,
        green: 232,
        blue: 242,
    });
}