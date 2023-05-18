use core::intrinsics::fabsf32;
use synapse::framebuffer::FramebufferInfo;
use synapse::framebuffer::PixelFormat;
use synapse::framebuffer::Color;

use crate::text_based_interface::primitive::{Point, Primitive};

pub struct FramebufferWriter {
    pub info: FramebufferInfo,
}

impl FramebufferWriter {
    pub fn new(info: FramebufferInfo) -> Self {
        Self {
            info
        }
    }

    pub fn draw_primitive(&self, buffer: &mut [u8], primitive: Primitive, color: Color) {
        match primitive {
            Primitive::Line(a, b) => { self.draw_line(buffer, a, b, color); }
            Primitive::Quad(a, b) => { self.draw_quad(buffer, a, b, color); }
            Primitive::Disk(a, r) => { self.draw_disk(buffer, a, r, color); }
            Primitive::Circle(a, r) => { self.draw_circle(buffer, a, r, color); }
            Primitive::Ellipse(a, b) => { self.draw_ellipse(buffer, a, b, color); }
            Primitive::BezierQuad(_, _, _) => {}
            Primitive::BezierCubic(_, _, _, _) => {}
        }
    }

    fn draw_pixel(&self, buffer: &mut [u8], x: usize, y: usize, color: Color) {
        if x < self.info.width && y < self.info.height {
            if self.info.pixel_format == PixelFormat::Rgb {
                buffer[(x + ((self.info.height - 1) - y) * self.info.stride) * self.info.bytes_per_pixel + 0] = color.red;
                buffer[(x + ((self.info.height - 1) - y) * self.info.stride) * self.info.bytes_per_pixel + 1] = color.green;
                buffer[(x + ((self.info.height - 1) - y) * self.info.stride) * self.info.bytes_per_pixel + 2] = color.blue;
            } else {
                buffer[(x + ((self.info.height - 1) - y) * self.info.stride) * self.info.bytes_per_pixel + 0] = color.blue;
                buffer[(x + ((self.info.height - 1) - y) * self.info.stride) * self.info.bytes_per_pixel + 1] = color.green;
                buffer[(x + ((self.info.height - 1) - y) * self.info.stride) * self.info.bytes_per_pixel + 2] = color.red;
            }
        }
    }

    fn draw_line(&self, buffer: &mut [u8], a: Point, b: Point, color: Color) {
        let mut x = a.x as isize;
        let mut y = a.y as isize;
        let x0 = a.x as isize;
        let y0 = a.y as isize;
        let x1 = b.x as isize;
        let y1 = b.y as isize;

        let dx = unsafe { fabsf32((x1 - x0) as f32) as isize };
        let sx: isize = if x0 < x1 { 1 } else { -1 };
        let dy = unsafe { -fabsf32((y1 - y0) as f32) as isize };
        let sy: isize = if y0 < y1 { 1 } else { -1 };

        let mut err = dx + dy;

        while x != x1 || y != y1 {
            self.draw_pixel(buffer, x as usize, y as usize, color);
            let e2 = 2 * err;
            if dy <= e2 {
                err += dy;
                x += sx;
            }
            if e2 <= dx {
                err += dx;
                y += sy;
            }
        }
    }

    fn draw_quad(&self, buffer: &mut [u8], a: Point, b: Point, color: Color) {
        let width = b.x - a.x;
        let height = b.y - a.y;

        for j in 0..height {
            for i in 0..width {
                let x = a.x + i;
                let y = a.y + j;

                self.draw_pixel(buffer, x, y, color);
            }
        }
    }

    fn draw_disk(&self, buffer: &mut [u8], a: Point, r: usize, color: Color) {
        let radius_squared = r * r;

        for i in 0..r {
            for j in 0..r {
                if i * i + j * j <= radius_squared {
                    let x = a.x;
                    let y = a.y;

                    self.draw_pixel(buffer, x + i, y + j, color);

                    if x as isize - i as isize >= 0 { self.draw_pixel(buffer, x - i, y + j, color); }
                    if x as isize - i as isize >= 0 && y as isize - j as isize >= 0 { self.draw_pixel(buffer, x - i, y - j, color); }
                    if y as isize - j as isize >= 0 { self.draw_pixel(buffer, x + i, y - j, color); }
                }
            }
        }
    }

    fn draw_circle(&self, buffer: &mut [u8], a: Point, r: usize, color: Color) {
        let mut x: isize = -(r as isize);
        let mut y: isize = 0;
        let mut rad: isize = r as isize;

        let mut err: isize = 2 - 2 * rad;
        loop {
            self.draw_pixel(buffer, (a.x as isize + y) as usize, (a.y as isize + x) as usize, color);

            if a.x as isize - x >= 0 { self.draw_pixel(buffer, (a.x as isize - x) as usize, (a.y as isize + y) as usize, color); }
            if a.x as isize - y >= 0 && a.y as isize - x >= 0 { self.draw_pixel(buffer, (a.x as isize - y) as usize, (a.y as isize - x) as usize, color); }
            if a.y as isize - y >= 0 { self.draw_pixel(buffer, (a.x as isize + x) as usize, (a.y as isize - y) as usize, color); }

            rad = err;
            if rad <= y {
                y += 1;
                err += 2 * y + 1;
            }

            if rad > x || err > y {
                x += 1;
                err += 2 * x + 1;
            }

            if x >= 0 { break; }
        }
    }

    fn draw_ellipse(&self, buffer: &mut [u8], a: Point, b: Point, color: Color) {}
}