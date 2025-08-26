#![no_std]
#![no_main]
#![feature(abi_x86_interrupt)]

use core::panic::PanicInfo;

use common::{
    addr::{Page, PhysFrame},
    BootInfo,
};

use crate::{
    arch::{
        gdt::initialize_gdt,
        interrupts::{enable_interrupts, idt::initialize_idt, pic::initialize_pic},
    },
    kmem::phys::PhysFrameAllocator,
};

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

    kprintln!("Hello from UgoOS.");

    initialize_gdt();
    initialize_idt();
    initialize_pic();
    enable_interrupts();
    kprintln!("Interrupts initialized.");

    let mut phys_allocator = PhysFrameAllocator::new(boot_info.mem_regions);

    let mut frames: [PhysFrame; 10] = [PhysFrame::from_base_u64(0); 10];

    for i in 0..10 {
        frames[i] = phys_allocator
            .alloc_frame()
            .expect("allocation should succeed!");
        kprintln!("{}", frames[i])
    }

    for i in 0..10 {
        phys_allocator.free_frame(frames[i]);
    }

    loop {}
}
