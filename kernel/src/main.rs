#![no_std]
#![no_main]
#![feature(abi_x86_interrupt)]

use core::panic::PanicInfo;

use common::{
    addr::{Address, VirtAddr},
    BootInfo, PHYSMEM_START,
};

use crate::{
    arch::{
        gdt::initialize_gdt,
        interrupts::{enable_interrupts, idt::initialize_idt, pic::initialize_pic},
    },
    kmem::{heap::KernelHeap, page::KernelPageTables, phys::PhysFrameAllocator},
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
    kprintln!("{}", boot_info.kernel_addrs);

    initialize_gdt();
    initialize_idt();
    initialize_pic();
    enable_interrupts();
    kprintln!("Interrupts initialized.");

    let page_tables = KernelPageTables::new();
    // Grab an entry to see if this works.
    let addr = VirtAddr::new(PHYSMEM_START);
    let entry = page_tables
        .get_entry(addr)
        .expect("this function should exist in the page tables");
    kprintln!("{}", entry);

    let phys_allocator = PhysFrameAllocator::new(boot_info.mem_regions);
    phys_allocator.print_stats();

    let heap = KernelHeap::new(boot_info.kernel_addrs);

    loop {}
}
