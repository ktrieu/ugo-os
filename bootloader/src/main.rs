#![no_main]
#![no_std]
#![feature(abi_efiapi)]

extern crate alloc;

use core::fmt::Write;
use core::slice;

use common::KMEM_START;
use common::PAGE_SIZE;
use graphics::{Console, Framebuffer};
use mem::valloc::VirtualAllocator;
use uefi::prelude::*;

mod fs;
mod graphics;
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
    frame: &'a mut FrameAllocator<'a, I>,
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
    writeln!(
        console,
        "{} bytes of memory detected.",
        frame.total_physical_memory()
    )
    .unwrap();

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

    let mut virt = VirtualAllocator::new(KMEM_START);

    let (phys_mem_offset, page_table, page_table_addr) = create_page_table(&mut frame, &mut virt);
    writeln!(
        console,
        "Mapping physical memory starting at {:#x}",
        phys_mem_offset
    )
    .unwrap();
    writeln!(
        console,
        "Creating kernel page table at {:#x}",
        page_table_addr
    )
    .unwrap();

    loop {}
}
