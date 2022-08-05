#![no_main]
#![no_std]
#![feature(abi_efiapi)]

extern crate alloc;

use core::fmt::Write;

use uefi::{prelude::*, table::boot::MemoryType};

mod fs;
mod mem;

use mem::mem_map;

#[entry]
fn uefi_main(_handle: Handle, mut system_table: SystemTable<Boot>) -> Status {
    uefi_services::init(&mut system_table).unwrap();

    writeln!(system_table.stdout(), "Hello from ugoOS!!").unwrap();

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

    let mem_map = mem_map::get_memory_map(system_table.boot_services())
        .expect("Failed to retrieve memory map.")
        .clone();

    for descriptor in mem_map
        .iter()
        .filter(|descriptor| descriptor.ty == MemoryType::CONVENTIONAL)
    {
        writeln!(
            system_table.stdout(),
            "P: {:#x} V: {:#x} | {} pages ({:?}) ",
            descriptor.phys_start,
            descriptor.virt_start,
            descriptor.page_count,
            descriptor.ty
        )
        .unwrap();
    }

    loop {}
}
