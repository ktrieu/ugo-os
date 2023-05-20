#![no_std]
#![no_main]

use core::panic::PanicInfo;

use common::BootInfo;

use crate::arch::gdt::initialize_gdt;

#[macro_use]
mod kprintln;

mod arch;
mod framebuffer;

#[panic_handler]
fn panic(info: &PanicInfo) -> ! {
    kprintln!("KERNEL PANIC: {}", info);
    loop {}
}

#[no_mangle]
pub extern "C" fn _start(boot_info: &'static mut BootInfo) -> ! {
    kprintln::init_kprintln(&boot_info.framebuffer);

    kprintln!("Hello from UgoOS.");

    initialize_gdt();
    kprintln!("GDT initialized.");

    loop {}
}
