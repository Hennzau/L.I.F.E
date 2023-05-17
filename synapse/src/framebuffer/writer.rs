use crate::framebuffer::FramebufferInfo;
use crate::framebuffer::PixelFormat;
use crate::framebuffer::Color;

pub struct FramebufferWriter {
    info: FramebufferInfo,
}

impl FramebufferWriter {
    pub fn new(info: FramebufferInfo) -> Self {
        Self {
            info
        }
    }

    pub fn write_pixel(&self, buffer: &mut [u8], x: usize, y: usize, color: Color) -> &Self {
        if x < self.info.width && y < self.info.height {
            if self.info.pixel_format == PixelFormat::Rgb {
                buffer[(x + y * self.info.stride) * self.info.bytes_per_pixel + 0] = color.red;
                buffer[(x + y * self.info.stride) * self.info.bytes_per_pixel + 1] = color.green;
                buffer[(x + y * self.info.stride) * self.info.bytes_per_pixel + 2] = color.blue;
            } else {
                buffer[(x + y * self.info.stride) * self.info.bytes_per_pixel + 0] = color.blue;
                buffer[(x + y * self.info.stride) * self.info.bytes_per_pixel + 1] = color.green;
                buffer[(x + y * self.info.stride) * self.info.bytes_per_pixel + 2] = color.red;
            }
        }

        &self
    }

    pub fn draw_quad(&self, buffer: &mut [u8], x: usize, y: usize, width: usize, height: usize, color: Color) -> &Self {
        for i in 0..width {
            for j in 0..height {
                self.write_pixel(buffer, x + i, y + j, color);
            }
        }

        &self
    }

    pub fn draw_centered_quad(&self, buffer: &mut [u8], x: usize, y: usize, width: usize, height: usize, color: Color) -> &Self {
        let half_width = width / 2 as usize;
        let half_height = height / 2 as usize;

        let left_x = if x as isize - half_width as isize >= 0 {
            x - half_width
        } else {
            0
        };

        let up_y = if y as isize - half_height as isize >= 0 {
            y - half_height
        } else {
            0
        };

        self.draw_quad(buffer, left_x, up_y, width, height, color)
    }

    pub fn draw_disk(&self, buffer: &mut [u8], x: usize, y: usize, radius: usize, color: Color) -> &Self {
        let radius_squared = radius * radius;

        for i in 0..radius {
            for j in 0..radius {
                if i * i + j * j <= radius_squared {
                    self.write_pixel(buffer, x + i, y + j, color);

                    if x as isize - i as isize >= 0 { self.write_pixel(buffer, x - i, y + j, color); }
                    if x as isize - i as isize >= 0 && y as isize - j as isize >= 0 { self.write_pixel(buffer, x - i, y - j, color); }
                    if y as isize - j as isize >= 0 { self.write_pixel(buffer, x + i, y - j, color); }
                }
            }
        }

        &self
    }

    fn draw_line_sud_east(&self, buffer: &mut [u8], start: (usize, usize), end: (usize, usize), radius: usize, color: Color) -> &Self {
        let (x_0, y_0) = start;
        let (x_1, y_1) = end;

        let mut x = x_0;
        let mut y = y_0;

        let (dx, dy) = (x_1 - x_0, y_1 - y_0);

        self.draw_disk(buffer, x_0, y_0, radius, color);
        self.draw_disk(buffer, x_1, y_1, radius, color);

        if dx >= dy {
            for i in 1..dx {
                let t: f32 = 0.5 + dy as f32 * i as f32 / (dx as f32);
                x = x_0 + i;
                y = y_0 + t as usize;

                self.draw_disk(buffer, x, y, radius, color);
            }
        } else {
            for j in 1..dy {
                let t: f32 = 0.5 + dx as f32 * j as f32 / (dy as f32);
                y = y_0 + j;
                x = x_0 + t as usize;

                self.draw_disk(buffer, x, y, radius, color);
            }
        }

        &self
    }

    fn draw_line_north_east(&self, buffer: &mut [u8], start: (usize, usize), end: (usize, usize), radius: usize, color: Color) -> &Self {
        let (x_0, y_0) = start;
        let (x_1, y_1) = end;

        let mut x = x_0;
        let mut y = y_0;

        let (dx, dy) = (x_1 - x_0, y_0 - y_1);

        self.draw_disk(buffer, x_0, y_0, radius, color);
        self.draw_disk(buffer, x_1, y_1, radius, color);

        if dx >= dy {
            for i in 1..dx {
                let t: f32 = 0.5 + dy as f32 * i as f32 / (dx as f32);
                x = x_0 + i;
                y = y_0 - t as usize;

                self.draw_disk(buffer, x, y, radius, color);
            }
        } else {
            for j in 1..dy {
                let t: f32 = 0.5 + dx as f32 * j as f32 / (dy as f32);
                y = y_0 - j;
                x = x_0 + t as usize;

                self.draw_disk(buffer, x, y, radius, color);
            }
        }

        &self
    }

    pub fn draw_line(&self, buffer: &mut [u8], start: (usize, usize), end: (usize, usize), radius: usize, color: Color) -> &Self {
        let (x_0, y_0) = start;
        let (x_1, y_1) = end;

        if x_1 >= x_0 && y_1 >= y_0 {
            return self.draw_line_sud_east(buffer, start, end, radius, color);
        } else if x_1 >= x_0 && y_0 >= y_1 {
            return self.draw_line_north_east(buffer, start, end, radius, color);
        } else if x_0 >= x_1 && y_1 >= y_0 {
            return self.draw_line_north_east(buffer, end, start, radius, color);
        } else if x_0 >= x_1 && y_0 >= y_1 {
            return self.draw_line_sud_east(buffer, end, start, radius, color);
        }

        &self
    }
}