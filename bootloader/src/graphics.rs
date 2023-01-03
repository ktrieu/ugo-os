use uefi::{
    prelude::BootServices,
    proto::console::gop::{GraphicsOutput, ModeInfo, PixelFormat},
    table::boot::ScopedProtocol,
};

use font8x8::legacy::BASIC_LEGACY;

pub fn locate_gop<'a>(
    boot_services: &'a BootServices,
) -> Result<ScopedProtocol<GraphicsOutput>, uefi::Error> {
    let handle = boot_services.get_handle_for_protocol::<GraphicsOutput>()?;
    boot_services.open_protocol_exclusive(handle)
}

pub struct Framebuffer {
    addr: *mut u8,
    mode: ModeInfo,
}

#[derive(Debug)]
pub enum FramebufferError {
    NoModes,
    ModeSetFailed(uefi::Error),
}

// We only support BGR formats, which are always 4bpp
const BYTES_PER_PIXEL: u32 = 4;

impl Framebuffer {
    pub fn new(gop: &mut ScopedProtocol<GraphicsOutput>) -> Result<Framebuffer, FramebufferError> {
        // Just grab the RGB mode with the biggest combined area
        let selected_mode = gop
            .modes()
            .filter(|m| matches!(m.info().pixel_format(), PixelFormat::Bgr))
            .max_by(|a, b| {
                let a_area = a.info().resolution().0 * a.info().resolution().1;
                let b_area = b.info().resolution().0 * b.info().resolution().1;

                a_area.cmp(&b_area)
            })
            .ok_or(FramebufferError::NoModes)?;

        gop.set_mode(&selected_mode)
            .map_err(|err| FramebufferError::ModeSetFailed(err))?;

        Ok(Framebuffer {
            addr: gop.frame_buffer().as_mut_ptr(),
            mode: selected_mode.info().clone(),
        })
    }

    fn stride(&self) -> u32 {
        self.mode.stride().try_into().unwrap()
    }

    fn coords_to_offset(&self, x: u32, y: u32) -> isize {
        if x >= self.width() || y >= self.height() {
            panic!(
                "Framebuffer coords ({}, {}) exceeded dimensions ({}, {})",
                x,
                y,
                self.width(),
                self.height()
            );
        }
        ((y * self.stride() * BYTES_PER_PIXEL) + x * BYTES_PER_PIXEL)
            .try_into()
            .unwrap()
    }

    pub fn write(&mut self, x: u32, y: u32, color: [u8; 3]) {
        // We should check that x and y don't overrun the bounds later
        let offset = self.coords_to_offset(x, y);
        unsafe {
            // We're only accepting BGR modes, so B, G, and R are the first three bytes
            self.addr.offset(offset + 2).write_volatile(color[0]);
            self.addr.offset(offset + 1).write_volatile(color[1]);
            self.addr.offset(offset).write_volatile(color[2]);
        }
    }

    pub fn read(&mut self, x: u32, y: u32) -> [u8; 3] {
        let offset = self.coords_to_offset(x, y);
        unsafe {
            [
                self.addr.offset(offset + 2).read_volatile(),
                self.addr.offset(offset + 1).read_volatile(),
                self.addr.offset(offset).read_volatile(),
            ]
        }
    }

    pub fn copy_row(&mut self, src_y: u32, dst_y: u32) {
        for x in 0..self.width() {
            let color = self.read(x, src_y);
            self.write(x, dst_y, color);
        }
    }

    pub fn clear_row(&mut self, y: u32) {
        for x in 0..self.width() {
            self.write(x, y, [0, 0, 0]);
        }
    }

    pub fn width(&self) -> u32 {
        self.mode.resolution().0.try_into().unwrap()
    }

    pub fn height(&self) -> u32 {
        self.mode.resolution().1.try_into().unwrap()
    }
}

const FONT_SCALE: u32 = 1;
const CHARACTER_SIZE: u32 = 8 * FONT_SCALE;
// Pad the console in by a few pixels so things aren't right up against the edge of the screen
const PADDING: u32 = 4;
const LINE_SPACING: u32 = 4;
const CHAR_SPACING: u32 = 1;

pub struct Console {
    framebuffer: Framebuffer,
    cwidth: u32,
    cheight: u32,
    cx: u32,
    cy: u32,
}

impl<'a> Console {
    pub fn new(gop: &mut ScopedProtocol<GraphicsOutput>) -> Result<Console, FramebufferError> {
        let framebuffer = Framebuffer::new(gop)?;
        let width = (framebuffer.width() - (2 * PADDING)) / (CHARACTER_SIZE + CHAR_SPACING);
        let height = (framebuffer.height() - (2 * PADDING)) / (CHARACTER_SIZE + LINE_SPACING);

        Ok(Console {
            framebuffer: framebuffer,
            cwidth: width,
            cheight: height,
            cx: 0,
            cy: 0,
        })
    }

    fn char_to_framebuffer(&self, cx: u32, cy: u32) -> (u32, u32) {
        (
            cx * (CHARACTER_SIZE + CHAR_SPACING) + PADDING,
            cy * (CHARACTER_SIZE + LINE_SPACING) + PADDING,
        )
    }

    fn newline(&mut self) {
        self.cx = 0;
        self.cy += 1;
        if self.cy >= self.cheight {
            self.cy = self.cheight - 1;
            for row in 1..self.cheight {
                let (_, src_frame_y) = self.char_to_framebuffer(0, row);
                let (_, dst_frame_y) = self.char_to_framebuffer(0, row - 1);
                for row_y in 0..CHARACTER_SIZE {
                    self.framebuffer
                        .copy_row(src_frame_y + row_y, dst_frame_y + row_y);
                }
            }
            // And clear our new row
            let (_, last_row_frame_y) = self.char_to_framebuffer(0, self.cheight - 1);
            for y in 0..CHARACTER_SIZE {
                self.framebuffer.clear_row(last_row_frame_y + y);
            }
        }
    }

    fn draw_glyph(&mut self, glyph: &[u8; 8]) {
        let (frame_x, frame_y) = self.char_to_framebuffer(self.cx, self.cy);
        for glyph_x in 0..CHARACTER_SIZE {
            for glyph_y in 0..CHARACTER_SIZE {
                let font_y: usize = (glyph_y / FONT_SCALE).try_into().unwrap();
                let font_x = glyph_x / FONT_SCALE;
                let row: u8 = glyph[font_y];
                let should_draw: bool = row >> font_x & 1 != 0;
                if should_draw {
                    self.framebuffer
                        .write(frame_x + glyph_x, frame_y + glyph_y, [32, 194, 14]);
                }
            }
        }
    }

    pub fn putchar(&mut self, c: u8) {
        match c {
            b'\n' => self.newline(),
            _ => {
                match BASIC_LEGACY.get::<usize>(c.into()) {
                    Some(bytes) => self.draw_glyph(bytes),
                    None => self.draw_glyph(BASIC_LEGACY.get::<usize>(b'?'.into()).unwrap()),
                };
                self.cx += 1;
                if self.cx >= self.cwidth {
                    self.newline();
                }
            }
        };
    }
}

impl core::fmt::Write for Console {
    fn write_str(&mut self, s: &str) -> core::fmt::Result {
        for b in s.as_bytes() {
            self.putchar(*b);
        }

        Ok(())
    }
}

unsafe impl<'a> Send for Console {}
