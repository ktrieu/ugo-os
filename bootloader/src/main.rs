#![no_main]
#![no_std]

use core::arch::asm;
use core::panic::PanicInfo;

use addr::VirtAddr;
use common::PAGE_SIZE;
use loader::LoaderResult;
use uefi::prelude::*;

#[macro_use]
mod logger;

mod addr;
mod frame;
mod fs;
mod graphics;
mod loader;
mod mappings;
mod page;

use uefi::table::boot::MemoryType;
use xmas_elf::ElfFile;

use crate::frame::FrameAllocator;
use crate::loader::Loader;
use crate::mappings::Mappings;

#[panic_handler]
fn panic(info: &PanicInfo) -> ! {
    bootlog!("{}", info);

    loop {}
}

fn init_logger(boot_services: &BootServices) {
    let mut gop = graphics::locate_gop(boot_services).expect("Failed to locate graphics protocol.");

    logger::logger_init(&mut gop);
}

fn read_kernel_file(boot_services: &BootServices) -> &'static [u8] {
    let mut sfs = fs::locate_sfs(boot_services).expect("Failed to locate filesystem protocol.");

    let mut root_volume = fs::open_root_volume(&mut sfs).expect("Failed to open root volume.");
    let mut kernel_file =
        fs::open_kernel_file(&mut root_volume).expect("Failed to open kernel file.");

    fs::read_file_data(boot_services, &mut kernel_file).expect("Failed to read kernel file.")
}

fn load_kernel(mappings: &mut Mappings, kernel_data: &[u8]) -> LoaderResult<VirtAddr> {
    let loader = Loader::new(kernel_data)?;

    loader.load_kernel(mappings)
}

// We grab at least 256 frames (1 GB) of physical memory for boot purposes
const MIN_BOOT_PHYS_FRAMES: u64 = 256;

#[entry]
fn uefi_main(handle: Handle, system_table: SystemTable<Boot>) -> Status {
    init_logger(&system_table.boot_services());

    bootlog!("Hello from ugoOS!");

    let file_data = read_kernel_file(&system_table.boot_services());

    bootlog!(
        "Kernel file loaded. File size: {}. ELF header: {:x?}.",
        file_data.len(),
        &file_data[0..4]
    );

    let (_, memory_map) = system_table.exit_boot_services();

    // DEBUG: Print memory map
    for d in memory_map
        .entries()
        .filter(|d| d.ty == MemoryType::CONVENTIONAL)
    {
        bootlog!(
            "({:#016x}-{:#016x}) {:?}",
            d.phys_start,
            d.phys_start + (PAGE_SIZE * d.page_count),
            d.ty,
        )
    }

    let mut frame_allocator = FrameAllocator::new(&memory_map, MIN_BOOT_PHYS_FRAMES);
    bootlog!(
        "Reserved physical memory for boot. ({:#016x}-{:#016x})",
        frame_allocator.alloc_start().as_u64(),
        frame_allocator.alloc_end().as_u64()
    );

    let mut page_mappings = Mappings::new(&mut frame_allocator);
    page_mappings.map_physical_memory(&memory_map, &mut frame_allocator);
    page_mappings.identity_map_fn(uefi_main as *const (), &mut frame_allocator);

    let virt_entrypoint = match load_kernel(&mut page_mappings, file_data) {
        Ok(loader) => loader,
        Err(err) => {
            panic!("Kernel load error: {}", err)
        }
    };

    bootlog!("Kernel entrypoint: {:#016x}", virt_entrypoint.as_u64());

    // Fasten your seatbelts.
    unsafe {
        asm!(
            "mov cr3, {addr}",
            addr = in(reg) page_mappings.level_4_phys_addr().as_u64()
        )
    }

    loop {}
}
