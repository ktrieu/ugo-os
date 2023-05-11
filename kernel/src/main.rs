#![no_std]
#![no_main]

use core::{
    panic::PanicInfo,
    ptr::{read_volatile, write_volatile},
};

static mut XYZ: [u8; 0x1000] = [0; 0x1000];

#[panic_handler]
fn panic(_info: &PanicInfo) -> ! {
    loop {}
}

#[no_mangle]
pub extern "C" fn _start() -> ! {
    loop {
        unsafe {
            for c in XYZ {
                read_volatile(&XYZ);
            }
        }
    }
}
