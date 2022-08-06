use uefi::{
    prelude::BootServices,
    proto::console::{
        gop::{GraphicsOutput, ModeInfo, PixelFormat},
        text::Color,
    },
};

pub fn locate_gop<'a>(boot_services: &'a BootServices) -> Result<&mut GraphicsOutput, uefi::Error> {
    boot_services
        .locate_protocol::<GraphicsOutput>()
        .map(|graphics_protocol| unsafe { &mut *graphics_protocol.get() })
}

pub struct Framebuffer {
    addr: *mut u8,
    mode: ModeInfo,
}

#[derive(Debug)]
pub enum FrameBufferError {
    NoModes,
    ModeSetFailed(uefi::Error),
}

// We only support BGR formats, which are always 4bpp
const BYTES_PER_PIXEL: u8 = 4;

impl Framebuffer {
    pub fn new(gop: &mut GraphicsOutput) -> Result<Framebuffer, FrameBufferError> {
        // Just grab the RGB mode with the biggest combined area
        let selected_mode = gop
            .modes()
            .filter(|m| matches!(m.info().pixel_format(), PixelFormat::Bgr))
            .max_by(|a, b| {
                let a_area = a.info().resolution().0 * a.info().resolution().1;
                let b_area = b.info().resolution().0 * b.info().resolution().1;

                a_area.cmp(&b_area)
            })
            .ok_or(FrameBufferError::NoModes)?;

        gop.set_mode(&selected_mode)
            .map_err(|err| FrameBufferError::ModeSetFailed(err))?;

        Ok(Framebuffer {
            addr: gop.frame_buffer().as_mut_ptr(),
            mode: selected_mode.info().clone(),
        })
    }

    pub fn write(&mut self, x: usize, y: usize, color: [u8; 3]) {
        // We should check that x and y don't overrun the bounds later
        let offset: isize = ((y * self.mode.stride() * 4) + x * 4).try_into().unwrap();
        unsafe {
            // We're only accepting BGR modes, so B, G, and R are the first three bytes
            self.addr.offset(offset).write_volatile(color[2]);
            self.addr.offset(offset + 1).write_volatile(color[1]);
            self.addr.offset(offset + 2).write_volatile(color[0]);
        }
    }
}
