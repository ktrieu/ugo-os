#![no_std]
#![no_main]

use core::panic::PanicInfo;

use common::{BootInfo, PAGE_SIZE};

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

    for region in &*boot_info.mem_regions {
        let ty_code = match region.ty {
            common::RegionType::Usable => "U",
            common::RegionType::Allocated => "A",
            common::RegionType::Bootloader => "B",
        };

        kprintln!(
            "{ty_code}: {:#016x} - {:#016x} ({} pages)",
            region.start,
            region.start + (PAGE_SIZE * region.pages),
            region.pages
        );
    }

    initialize_gdt();

    kprintln!("GDT initialized.");

    loop {}
}
