#![no_std]
#![no_main]
#![feature(abi_x86_interrupt)]

use core::{arch::asm, panic::PanicInfo};

use common::BootInfo;

use crate::arch::{
    gdt::initialize_gdt,
    interrupts::{idt::initialize_idt, with_interrupts_disabled},
};

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

    with_interrupts_disabled(|| {
        initialize_gdt();
        initialize_idt();
    });
    kprintln!("Interrupts initialized.");

    loop {}
}
