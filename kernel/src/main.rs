#![no_std]
#![no_main]
#![feature(abi_x86_interrupt)]

use core::panic::PanicInfo;

use alloc::vec::Vec;
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

    // Comment out the allocation test code for now.
    let mut allocated = 0;

    for _ in 0..100 {
        let n = 1024;
        let mut test = Vec::<u8>::with_capacity(n);
        for _ in 0..n {
            test.push(b'a');
        }
        allocated += n;
        kprintln!("allocated {allocated} bytes")
    }

    loop {}
}
