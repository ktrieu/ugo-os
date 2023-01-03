#![no_main]
#![no_std]
#![feature(abi_efiapi)]

use core::panic::PanicInfo;
use core::slice;

use uefi::prelude::*;

#[macro_use]
mod logger;

mod fs;
mod graphics;

use uefi::table::boot::MemoryMapSize;
use uefi::table::boot::MemoryType;

fn get_memory_map_size(boot_services: &BootServices) -> usize {
    let MemoryMapSize {
        entry_size,
        mut map_size,
    } = boot_services.memory_map_size();
    // Allocating memory might add a few descriptors, so just to be safe, reserve a few more
    map_size += 2;

    entry_size * map_size
}

#[panic_handler]
fn panic(info: &PanicInfo) -> ! {
    bootlog!("{}", info);

    loop {}
}

fn init_logger(boot_services: &BootServices) {
    let mut gop = graphics::locate_gop(boot_services).expect("Failed to locate graphics protocol.");

    logger::logger_init(&mut gop);
}

fn read_kernel_file(boot_services: &BootServices) -> &[u8] {
    let mut sfs = fs::locate_sfs(boot_services).expect("Failed to locate filesystem protocol.");

    let mut root_volume = fs::open_root_volume(&mut sfs).expect("Failed to open root volume.");
    let mut kernel_file =
        fs::open_kernel_file(&mut root_volume).expect("Failed to open kernel file.");

    fs::read_file_data(boot_services, &mut kernel_file).expect("Failed to read kernel file.")
}

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

    let mem_map_buffer_size = get_memory_map_size(system_table.boot_services());
    let mem_map_buffer = unsafe {
        let raw_buffer = system_table
            .boot_services()
            .allocate_pool(MemoryType::LOADER_DATA, mem_map_buffer_size)
            .expect("Could not allocate space for memory map.");
        slice::from_raw_parts_mut(raw_buffer, mem_map_buffer_size)
    };

    let (_, _descriptors) = system_table
        .exit_boot_services(handle, mem_map_buffer)
        .expect("Could not exit boot services.");

    loop {}
}
