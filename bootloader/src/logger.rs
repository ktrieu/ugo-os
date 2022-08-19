use conquer_once::spin::OnceCell;
use uefi::proto::console::gop::GraphicsOutput;

use crate::graphics::Console;

pub static LOGGER: OnceCell<spin::Mutex<Console>> = OnceCell::uninit();

macro_rules! bootlog {
    ($($args:tt)*) => {
        // IMPORTANT: We need to wrap this in a block so the lock gets unlocked. Macros don't introduce blocks on their own
        // so we have to do this ourselves.
        {
            let mut console = LOGGER.try_get().unwrap().lock();
            writeln!(console, $($args)*).unwrap()
        }
    };
}

pub fn logger_init(gop: &mut GraphicsOutput) {
    LOGGER.init_once(|| spin::Mutex::new(Console::new(gop).expect("Failed to create console.")));
}
