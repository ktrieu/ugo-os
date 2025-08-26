use common::FramebufferInfo;
use conquer_once::spin::OnceCell;

use crate::{framebuffer::TextFramebuffer, sync::InterruptSafeSpinlock};

pub static TEXT_FB: OnceCell<InterruptSafeSpinlock<TextFramebuffer>> = OnceCell::uninit();

macro_rules! kprintln {
    ($($args:tt)*) => {
        // IMPORTANT: We need to wrap this in a block so the lock gets unlocked. Macros don't introduce blocks on their own
        // so we have to do this ourselves.
        {
            use core::fmt::Write;
            let mut text_fb = $crate::kprintln::TEXT_FB.try_get().unwrap().lock();
            writeln!(text_fb, $($args)*).unwrap()
        }
    };
}

macro_rules! kprint {
    ($($args:tt)*) => {
        // IMPORTANT: We need to wrap this in a block so the lock gets unlocked. Macros don't introduce blocks on their own
        // so we have to do this ourselves.
        {
            use core::fmt::Write;
            let mut text_fb = $crate::kprintln::TEXT_FB.try_get().unwrap().lock();
            write!(text_fb, $($args)*).unwrap()
        }
    };
}

pub fn init_kprintln(info: &FramebufferInfo) {
    TEXT_FB.init_once(|| {
        let mut fb = TextFramebuffer::new(info);
        fb.clear();
        InterruptSafeSpinlock::new(fb)
    })
}
