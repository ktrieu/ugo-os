use common::{FramebufferFormat, FramebufferInfo};
use noto_sans_mono_bitmap::{get_raster, get_raster_width, FontWeight, RasterHeight};

/**
 * These suck. We should be double buffering and storing console characters
 * in a separate buffer, but that will have to wait until we get memory allocation
 * working.
 *
 * In the meantime, this should do to get output on the screen.
 */

pub struct Framebuffer {
    addr: *mut u8,
    height: usize,
    width: usize,
    stride: usize,
    format: FramebufferFormat,
}

#[derive(Clone, Copy)]
pub struct Color([u8; 3]);

impl Color {
    pub const BLACK: Color = Color([0, 0, 0]);
}

impl From<[u8; 3]> for Color {
    fn from(value: [u8; 3]) -> Self {
        Color(value)
    }
}

impl Framebuffer {
    pub fn new(info: &FramebufferInfo) -> Self {
        Self {
            addr: info.address,
            height: info.height,
            width: info.width,
            stride: info.stride,
            format: info.format,
        }
    }

    pub fn bpp(&self) -> usize {
        match self.format {
            FramebufferFormat::Bgr => 4,
        }
    }

    fn get_pixel_ptr(&self, x: usize, y: usize) -> *mut u8 {
        self.addr
            .wrapping_add(y * self.stride * self.bpp() + (x * self.bpp()))
    }

    // Safety: ptr must be a valid pointer to framebuffer memory.
    unsafe fn write_pixel_to_ptr(&mut self, ptr: *mut u8, mut c: Color) {
        match self.format {
            FramebufferFormat::Bgr => c.0.reverse(),
        };

        for (idx, byte) in c.0.iter().enumerate() {
            ptr.add(idx).write_volatile(*byte);
        }
    }

    pub fn put(&mut self, x: usize, y: usize, c: Color) {
        assert!(x < self.width);
        assert!(y < self.height);

        let pixel_ptr = self.get_pixel_ptr(x, y);

        // We've made sure that x and y are in bounds.
        unsafe {
            self.write_pixel_to_ptr(pixel_ptr, c);
        }
    }

    pub fn clear(&mut self, c: Color) {
        for y in 0..self.height {
            for x in 0..self.width {
                self.put(x, y, c);
            }
        }
    }
}

#[derive(Clone, Copy)]
pub struct CharPos(pub usize, pub usize);

pub struct TextFramebuffer {
    fb: Framebuffer,
    cursor: CharPos,
    line_length: usize,
    lines: usize,
    char_width: usize,
    char_height: usize,
}

impl TextFramebuffer {
    // It looks better if the text isn't right up on the edge of the screen
    // so pad it out from the edge by a few pixels.
    const BORDER_PADDING: usize = 5;
    // Pixels between lines of text.
    const LINE_GAP: usize = 5;

    // Font parameters:
    const RASTER_HEIGHT: RasterHeight = RasterHeight::Size16;
    const FONT_WEIGHT: FontWeight = FontWeight::Regular;

    fn pixel_coords(&self, char_pos: CharPos) -> (usize, usize) {
        (
            char_pos.0 * self.char_width + Self::BORDER_PADDING,
            char_pos.1 * (self.char_width + Self::LINE_GAP) + Self::BORDER_PADDING,
        )
    }

    pub fn new(info: &FramebufferInfo) -> Self {
        let fb = Framebuffer::new(info);

        let (usable_x, usable_y) = (
            fb.width - Self::BORDER_PADDING * 2,
            fb.height - Self::BORDER_PADDING * 2,
        );
        let char_width = get_raster_width(Self::FONT_WEIGHT, Self::RASTER_HEIGHT);
        let char_height: usize = RasterHeight::Size16.val();

        let lines = usable_y / (char_height + Self::LINE_GAP);
        let line_length = usable_x / char_width;

        Self {
            fb,
            cursor: CharPos(0, 0),
            line_length,
            lines,
            char_width,
            char_height,
        }
    }

    pub fn write_char(&mut self, pos: CharPos, c: char) {
        let (pixel_x, pixel_y) = self.pixel_coords(pos);
        let rasterized =
            get_raster(c, Self::FONT_WEIGHT, Self::RASTER_HEIGHT).unwrap_or_else(|| {
                // Fallback to a question mark if we use a character the font doesn't have.
                get_raster('?', Self::FONT_WEIGHT, Self::RASTER_HEIGHT)
                    .expect("No character found for '?'. This shouldn't happen.")
            });

        for char_x in 0..self.char_width {
            for char_y in 0..self.char_height {
                let byte = rasterized.raster()[char_y][char_x];
                self.fb.put(
                    pixel_x + char_x,
                    pixel_y + char_y,
                    Color([byte, byte, byte]),
                )
            }
        }
    }

    pub fn clear(&mut self) {
        self.fb.clear(Color::BLACK);
        self.cursor = CharPos(0, 0);
    }
}
