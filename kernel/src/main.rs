#![no_std]
#![no_main]
#![feature(abi_x86_interrupt)]

use core::panic::PanicInfo;

use alloc::boxed::Box;
use common::BootInfo;

use crate::{
    arch::{
        gdt::initialize_gdt,
        interrupts::{enable_interrupts, idt::initialize_idt, pic::initialize_pic},
    },
    kmem::KernelMemoryManager,
};

extern crate alloc;

#[macro_use]
mod kprintln;

mod arch;
mod framebuffer;
mod kmem;
mod sync;

#[panic_handler]
fn panic(info: &PanicInfo) -> ! {
    kprintln!("KERNEL PANIC: {}", info);
    loop {}
}

#[no_mangle]
pub extern "C" fn _start(boot_info: &'static mut BootInfo) -> ! {
    kprintln::init_kprintln(&boot_info.framebuffer);

    let kmm = KernelMemoryManager::new(boot_info);
    kmm.register_global();

    kprintln!("Hello from UgoOS.");
    kprintln!("{}", boot_info.kernel_addrs);

    initialize_gdt();
    initialize_idt();
    initialize_pic();
    enable_interrupts();
    kprintln!("Interrupts initialized.");

    let mut allocated = 0;

    let mut boxes: [Option<Box<[u8; 16]>>; 100] = [const { None }; 100];

    for i in 0..100 {
        boxes[i] = Some(Box::new([0; 16]));
        allocated += 16;
        kprintln!("allocated {allocated} bytes")
    }

    let mut freed = 0;
    for i in 0..100 {
        boxes[i] = None;
        freed += 16;
        kprintln!("freed {freed} bytes")
    }

    loop {}
}
