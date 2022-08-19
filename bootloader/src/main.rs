#![no_main]
#![no_std]
#![feature(abi_efiapi)]

use core::fmt::Write;
use core::panic::PanicInfo;
use core::slice;

use common::KMEM_START;
use common::PAGE_SIZE;
use loader::load_kernel;
use logger::LOGGER;
use mem::valloc::VirtualAllocator;
use uefi::prelude::*;

mod fs;
mod graphics;
mod loader;
#[macro_use]
mod logger;
mod mem;

use mem::frame::FrameAllocator;
use uefi::table::boot::MemoryDescriptor;
use uefi::table::boot::MemoryMapSize;
use uefi::table::boot::MemoryType;
use x86_64::structures::paging::OffsetPageTable;
use x86_64::structures::paging::PageTable;
use x86_64::VirtAddr;

fn get_memory_map_size(boot_services: &BootServices) -> usize {
    let MemoryMapSize {
        entry_size,
        mut map_size,
    } = boot_services.memory_map_size();
    // Allocating memory might add a few descriptors, so just to be safe, reserve a few more
    map_size += 2;

    entry_size * map_size
}

fn create_page_table<'a, I>(
    frame: &mut FrameAllocator<'a, I>,
    virt: &mut VirtualAllocator,
) -> (VirtAddr, OffsetPageTable<'static>, u64)
where
    I: ExactSizeIterator<Item = &'a MemoryDescriptor> + Clone,
{
    let num_frames = frame.total_physical_memory() / PAGE_SIZE;
    // Probably rework this later, but map all of our physical memory right at the start of virtual memory
    let phys_mem_offset = VirtAddr::new(
        virt.allocate(num_frames)
            .expect("Could not allocate virtual memory space for physical memory mapping."),
    );

    let new_frame = frame
        .allocate(1)
        .expect("No free frames for kernel page table.");

    unsafe {
        let page_ptr = new_frame as *mut PageTable;
        *page_ptr = PageTable::new();
        let page_table = OffsetPageTable::new(&mut *page_ptr, phys_mem_offset);
        (phys_mem_offset, page_table, new_frame)
    }
}

#[panic_handler]
fn panic(info: &PanicInfo) -> ! {
    bootlog!("{}", info);

    loop {}
}

#[entry]
fn uefi_main(handle: Handle, system_table: SystemTable<Boot>) -> Status {
    let gop = graphics::locate_gop(system_table.boot_services())
        .expect("Failed to locate graphics protocol.");

    logger::logger_init(gop);

    bootlog!("Hello from ugoOS!");

    let sfs = fs::locate_sfs(system_table.boot_services())
        .expect("Failed to locate filesystem protocol.");

    let mut root_volume = fs::open_root_volume(sfs).expect("Failed to open root volume.");
    let mut kernel_file =
        fs::open_kernel_file(&mut root_volume).expect("Failed to open kernel file.");
    let file = fs::read_file_data(system_table.boot_services(), &mut kernel_file)
        .expect("Failed to read kernel file.");

    bootlog!(
        "Kernel file loaded. File size: {}. ELF header: {:x?}.",
        file.len(),
        &file[0..4]
    );

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

    let mut frame = FrameAllocator::new(descriptors.clone());
    bootlog!(
        "{} bytes of memory detected.",
        frame.total_physical_memory()
    );

    bootlog!("Free memory segments:");
    for d in descriptors
        .clone()
        .filter(|d| d.ty == MemoryType::CONVENTIONAL)
    {
        bootlog!(
            "{:#x} - {:#x} ({} pages)",
            d.phys_start,
            d.phys_start + (PAGE_SIZE * d.page_count),
            d.page_count
        );
    }

    let mut virt = VirtualAllocator::new(KMEM_START);

    let (phys_mem_offset, mut page_table, page_table_addr) =
        create_page_table(&mut frame, &mut virt);

    bootlog!("Mapping physical memory starting at {:#x}", phys_mem_offset);
    bootlog!("Creating kernel page table at {:#x}", page_table_addr);

    load_kernel(file, &mut frame, &mut virt, &mut page_table).expect("Failed to load kernel!");

    bootlog!(
        "Kernel loaded.\nPage table:\n{:?}",
        page_table.level_4_table().iter().find(|e| !e.is_unused())
    );

    loop {}
}
