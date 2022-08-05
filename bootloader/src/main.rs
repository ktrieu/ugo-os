#![no_main]
#![no_std]
#![feature(abi_efiapi)]

extern crate alloc;

use core::fmt::Write;

use common::{RegionType, KMEM_START};
use uefi::prelude::*;

mod fs;
mod mem;

use mem::mem_map;
use mem::valloc;

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

    let mem_regions = mem_map::get_memory_map(system_table.boot_services())
        .expect("Failed to retrieve memory map.");

    for region in mem_regions
        .iter()
        .filter(|region| matches!(region.ty, RegionType::Usable))
    {
        writeln!(
            system_table.stdout(),
            "{:?} ({:#10x} - {:#10x})",
            region.ty,
            region.start,
            region.end
        )
        .unwrap();
    }

    loop {}
}
