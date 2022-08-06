#![no_main]
#![no_std]
#![feature(abi_efiapi)]

extern crate alloc;

use core::fmt::Write;
use core::slice;

use uefi::prelude::*;

mod fs;
mod mem;

use common::KMEM_START;
use mem::frame::FrameAllocator;
use mem::valloc;
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

#[entry]
fn uefi_main(handle: Handle, mut system_table: SystemTable<Boot>) -> Status {
    uefi_services::init(&mut system_table).unwrap();

    writeln!(system_table.stdout(), "Hello from ugoOS!").unwrap();

    let sfs = fs::locate_sfs(system_table.boot_services())
        .expect("Failed to locate filesystem protocol.");

    let mut root_volume = fs::open_root_volume(sfs).expect("Failed to open root volume.");
    let mut kernel_file =
        fs::open_kernel_file(&mut root_volume).expect("Failed to open kernel file.");
    let file = fs::read_file_data(system_table.boot_services(), &mut kernel_file)
        .expect("Failed to read kernel file.");

    writeln!(
        system_table.stdout(),
        "Kernel file loaded. File size: {}. ELF header: {:x?}.",
        file.len(),
        &file[0..4]
    )
    .unwrap();

    let mut valloc = valloc::VirtualAllocator::new(KMEM_START);

    // Test the virtual allocator
    let alloc_start_1 = valloc.allocate(2).unwrap();
    let alloc_start_2 = valloc.allocate(2).unwrap();

    writeln!(
        system_table.stdout(),
        "Allocated 2 virtual pages starting at {:#x}",
        alloc_start_1
    )
    .unwrap();

    writeln!(
        system_table.stdout(),
        "Allocated 2 virtual pages starting at {:#x}",
        alloc_start_2
    )
    .unwrap();

    let mem_map_buffer_size = get_memory_map_size(system_table.boot_services());
    let mem_map_buffer = unsafe {
        let raw_buffer = system_table
            .boot_services()
            .allocate_pool(MemoryType::LOADER_DATA, mem_map_buffer_size)
            .expect("Could not allocate space for memory map.");
        slice::from_raw_parts_mut(raw_buffer, mem_map_buffer_size)
    };

    let (runtime_table, descriptors) = system_table
        .exit_boot_services(handle, mem_map_buffer)
        .expect("Could not exit boot services.");

    let mut frame = FrameAllocator::new(descriptors);

    // Test the frame allocator
    let palloc_1 = frame.allocate(16).expect("Failed to allocate frames.");
    let palloc_2 = frame.allocate(1).expect("Failed to allocate frames");

    loop {}
}
