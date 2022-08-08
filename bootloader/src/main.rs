#![no_main]
#![no_std]
#![feature(abi_efiapi)]

extern crate alloc;

use core::fmt::Write;
use core::slice;

use common::PAGE_SIZE;
use graphics::{Console, Framebuffer};
use uefi::prelude::*;

mod fs;
mod graphics;
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

    let gop = graphics::locate_gop(system_table.boot_services())
        .expect("Failed to locate graphics protocol.");

    let mut framebuffer = Framebuffer::new(gop).expect("Could not create framebufffer.");
    let mut console = Console::new(&mut framebuffer);

    writeln!(console, "Hello from ugoOS!").unwrap();

    let sfs = fs::locate_sfs(system_table.boot_services())
        .expect("Failed to locate filesystem protocol.");

    let mut root_volume = fs::open_root_volume(sfs).expect("Failed to open root volume.");
    let mut kernel_file =
        fs::open_kernel_file(&mut root_volume).expect("Failed to open kernel file.");
    let file = fs::read_file_data(system_table.boot_services(), &mut kernel_file)
        .expect("Failed to read kernel file.");

    writeln!(
        console,
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
        console,
        "Allocated 2 virtual pages starting at {:#x}",
        alloc_start_1
    )
    .unwrap();

    writeln!(
        console,
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

    let (_, descriptors) = system_table
        .exit_boot_services(handle, mem_map_buffer)
        .expect("Could not exit boot services.");

    writeln!(console, "Free memory segments:").unwrap();
    for d in descriptors
        .clone()
        .filter(|d| d.ty == MemoryType::CONVENTIONAL)
    {
        writeln!(
            console,
            "{:#x} - {:#x} ({} pages)",
            d.phys_start,
            d.phys_start + (PAGE_SIZE * d.page_count),
            d.page_count
        )
        .unwrap();
    }

    let mut frame = FrameAllocator::new(descriptors);

    // Test the frame allocator
    let palloc_1 = frame.allocate(159).expect("Failed to allocate frames.");
    let palloc_2 = frame.allocate(1).expect("Failed to allocate frames");

    writeln!(console, "Allocated 16 frames at {:#x}", palloc_1).unwrap();
    writeln!(console, "Allocated 1 frame at {:#x}", palloc_2).unwrap();

    loop {}
}
