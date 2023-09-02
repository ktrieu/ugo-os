#![no_std]
#![no_main]
#![feature(abi_x86_interrupt)]

use core::panic::PanicInfo;

use common::BootInfo;

use crate::arch::{
    gdt::initialize_gdt,
    interrupts::{enable_interrupts, idt::initialize_idt, pic::initialize_pic},
};

#[macro_use]
mod kprintln;

mod arch;
mod framebuffer;
mod sync;

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
    initialize_idt();
    initialize_pic();
    enable_interrupts();
    kprintln!("Interrupts initialized.");

    loop {}
}
