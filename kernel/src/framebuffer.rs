use common::{FramebufferFormat, FramebufferInfo};

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
