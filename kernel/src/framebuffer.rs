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

    fn in_bounds(&self, x: usize, y: usize) -> bool {
        x < self.width && y < self.height
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
        assert!(self.in_bounds(x, y));

        let pixel_ptr = self.get_pixel_ptr(x, y);

        // We've made sure that x and y are in bounds.
        unsafe {
            self.write_pixel_to_ptr(pixel_ptr, c);
        }
    }

    pub fn clear_all(&mut self, c: Color) {
        self.clear_rows(0, self.height, c);
    }

    // Thanks to the whole linear framebuffer thing, we can copy entire contiguous sets of rows really fast
    // by reducing it to a linear memory copy.
    pub fn copy_rows(&mut self, src_row: usize, dst_row: usize, rows: usize) {
        // Check and make sure we're not copying past the bounds of the buffer.
        assert!(self.in_bounds(0, src_row));
        assert!(self.in_bounds(0, dst_row));
        assert!(self.in_bounds(0, src_row + rows - 1));
        assert!(self.in_bounds(0, dst_row + rows - 1));

        let src_ptr = self.get_pixel_ptr(0, src_row);
        let dst_ptr = self.get_pixel_ptr(0, dst_row);
        let len = rows * self.stride * self.bpp();

        unsafe {
            src_ptr.copy_to(dst_ptr, len);
        }
    }

    pub fn clear_rows(&mut self, start: usize, rows: usize, c: Color) {
        assert!(self.in_bounds(0, start));
        assert!(self.in_bounds(0, start + rows - 1));

        for y in start..start + rows {
            for x in 0..self.width {
                self.put(x, y, c);
            }
        }
    }
}

#[derive(Clone, Copy)]
pub struct CharPos(pub usize, pub usize);

impl CharPos {
    pub fn next_char(&self) -> CharPos {
        CharPos(self.0 + 1, self.1)
    }

    pub fn next_line(&self) -> CharPos {
        CharPos(0, self.1 + 1)
    }

    pub fn start_of_line(&self) -> CharPos {
        CharPos(0, self.1)
    }
}

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

    const CLEAR_COLOR: Color = Color::BLACK;

    fn line_height(&self) -> usize {
        self.char_height + Self::LINE_GAP
    }

    fn pixel_coords(&self, char_pos: CharPos) -> Option<(usize, usize)> {
        if self.is_past_line_end(char_pos) || self.is_past_last_line(char_pos) {
            None
        } else {
            Some((
                char_pos.0 * self.char_width + Self::BORDER_PADDING,
                char_pos.1 * self.line_height() + Self::BORDER_PADDING,
            ))
        }
    }

    fn is_past_line_end(&self, char_pos: CharPos) -> bool {
        char_pos.0 >= self.line_length
    }

    fn is_past_last_line(&self, char_pos: CharPos) -> bool {
        char_pos.1 >= self.lines
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

    pub fn put_char(&mut self, pos: CharPos, c: char) {
        let (pixel_x, pixel_y) = self
            .pixel_coords(pos)
            .expect("Character position out of bounds.");
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
        self.fb.clear_all(Self::CLEAR_COLOR);
        self.cursor = CharPos(0, 0);
    }

    fn advance_cursor(&mut self) {
        self.cursor = self.cursor.next_char();

        if self.is_past_line_end(self.cursor) {
            self.newline();
        }
    }

    fn shift_buffer_up(&mut self) {
        self.fb
            .copy_rows(self.line_height(), 0, self.fb.height - self.line_height());
    }

    fn carriage_return(&mut self) {
        self.cursor = self.cursor.start_of_line();
        self.fb.clear_rows(
            self.fb.height - self.line_height(),
            self.line_height(),
            Self::CLEAR_COLOR,
        );
    }

    fn newline(&mut self) {
        let next_line = self.cursor.next_line();

        if self.is_past_last_line(next_line) {
            // If we're at the end of the screen, we need to shift everything up, and then carriage return.
            self.shift_buffer_up();
            self.carriage_return();
        } else {
            self.cursor = next_line
        }
    }

    pub fn write_char(&mut self, char: char) {
        if char == '\n' {
            self.newline()
        } else {
            self.put_char(self.cursor, char);
            self.advance_cursor();
        }
    }

    pub fn write_text(&mut self, text: &str) {
        for char in text.chars() {
            self.write_char(char);
        }
    }
}

impl core::fmt::Write for TextFramebuffer {
    fn write_str(&mut self, s: &str) -> core::fmt::Result {
        self.write_text(s);

        Ok(())
    }
}

unsafe impl Send for TextFramebuffer {}
