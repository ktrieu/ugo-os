#![no_main]
#![no_std]

use core::arch::asm;
use core::panic::PanicInfo;

use common::{BootInfo, PAGE_SIZE};
use loader::{KernelAddresses, LoaderResult};
use uefi::prelude::*;

#[macro_use]
mod logger;

mod boot_info;
mod frame;
mod fs;
mod graphics;
mod loader;
mod mappings;
mod page;

use uefi::table::boot::MemoryType;

use crate::boot_info::create_boot_info;
use crate::frame::FrameAllocator;
use crate::loader::Loader;
use crate::logger::LOGGER;
use crate::mappings::Mappings;

use common::addr::Address;

#[panic_handler]
fn panic(info: &PanicInfo) -> ! {
    bootlog!("{}", info);

    loop {}
}

fn init_logger(boot_services: &BootServices) {
    let mut gop = graphics::locate_gop(boot_services).expect("Failed to locate graphics protocol.");

    logger::logger_init(&mut gop);

    let (width, height) = {
        let framebuffer = LOGGER.try_get().unwrap().lock().framebuffer();
        (framebuffer.width(), framebuffer.height())
    };

    bootlog!("Selected video mode ({}x{})", width, height);
}

fn read_kernel_file(boot_services: &BootServices) -> &'static [u8] {
    let mut sfs = fs::locate_sfs(boot_services).expect("Failed to locate filesystem protocol.");

    let mut root_volume = fs::open_root_volume(&mut sfs).expect("Failed to open root volume.");
    let mut kernel_file =
        fs::open_kernel_file(&mut root_volume).expect("Failed to open kernel file.");

    fs::read_file_data(boot_services, &mut kernel_file).expect("Failed to read kernel file.")
}

fn load_kernel(
    mappings: &mut Mappings,
    allocator: &mut FrameAllocator,
    kernel_data: &[u8],
) -> LoaderResult<KernelAddresses> {
    let mut loader = Loader::new(kernel_data)?;

    loader.load_kernel(mappings, allocator)
}

// We grab at least 256 frames (1 GB) of physical memory for boot purposes
const MIN_BOOT_PHYS_FRAMES: u64 = 256;

fn jump_to_kernel(
    level_4_address: u64,
    stack_top: u64,
    kernel_entry: u64,
    boot_info_ptr: *mut BootInfo,
) {
    // Fasten your seatbelts.
    unsafe {
        asm!(
            "
            mov cr3, {addr}
            mov rsp, {stack_top}
            jmp {entry}",
            addr = in(reg) level_4_address,
            stack_top = in(reg) stack_top,
            entry = in(reg) kernel_entry,
            in("rdi") boot_info_ptr
        )
    }
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

    let (_, mut memory_map) = system_table.exit_boot_services();
    memory_map.sort();

    let mut frame_allocator = FrameAllocator::new(&memory_map, MIN_BOOT_PHYS_FRAMES);
    bootlog!(
        "Reserved physical memory for boot. ({}-{})",
        frame_allocator.alloc_start(),
        frame_allocator.alloc_end()
    );

    let mut page_mappings = Mappings::new(&mut frame_allocator);
    page_mappings.map_physical_memory(&memory_map, &mut frame_allocator);
    page_mappings.identity_map_fn(jump_to_kernel as *const (), &mut frame_allocator);

    let addresses = match load_kernel(&mut page_mappings, &mut frame_allocator, file_data) {
        Ok(loader) => loader,
        Err(err) => {
            panic!("Kernel load error: {}", err)
        }
    };

    // This is kind of ugh, but whatever.
    let framebuffer = LOGGER.try_get().unwrap().lock().framebuffer();
    let boot_info_ptr = create_boot_info(
        &mut frame_allocator,
        &mut page_mappings,
        &framebuffer,
        memory_map,
    );

    bootlog!("Kernel entrypoint: {}", addresses.kernel_entry);

    jump_to_kernel(
        page_mappings.level_4_phys_addr().as_u64(),
        addresses.stack_top.as_u64(),
        addresses.kernel_entry.as_u64(),
        boot_info_ptr,
    );

    loop {}
}
