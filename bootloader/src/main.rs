#![no_main]
#![no_std]
#![feature(abi_efiapi)]

use core::fmt::Write;

use uefi::{
    prelude::*,
    proto::media::{
        file::{Directory, File, FileAttribute, FileHandle, FileMode},
        fs::SimpleFileSystem,
    },
    table::boot::{OpenProtocolAttributes, OpenProtocolParams, ScopedProtocol},
    CStr16,
};

fn open_root_volume(sfs: &mut SimpleFileSystem) -> Result<Directory, uefi::Error> {
    sfs.open_volume()
}

fn open_kernel_file(dir: &mut Directory) -> Result<FileHandle, uefi::Error> {
    // The open function only takes CStr16's, and converting it is sort of involved...
    let mut buf: [u16; 11] = [0; 11]; // 10 chars for the name, plus 1 for null terminator

    dir.open(
        CStr16::from_str_with_buf("ugo-os.elf", &mut buf).unwrap(),
        FileMode::Read,
        FileAttribute::VALID_ATTR,
    )
}

fn locate_sfs<'a>(
    handle: Handle,
    boot_services: &'a BootServices,
) -> Result<ScopedProtocol<'a, SimpleFileSystem>, uefi::Error> {
    boot_services.open_protocol::<SimpleFileSystem>(
        OpenProtocolParams {
            handle: handle,
            agent: handle,
            controller: None,
        },
        OpenProtocolAttributes::Exclusive,
    )
}

#[entry]
fn uefi_main(handle: Handle, mut system_table: SystemTable<Boot>) -> Status {
    uefi_services::init(&mut system_table).unwrap();

    writeln!(system_table.stdout(), "Hello from ugoOS!!!!").unwrap();

    let scoped_sfs = locate_sfs(handle, system_table.boot_services())
        .expect("Failed to locate filesystem protocol.");
    let sfs = unsafe { &mut *scoped_sfs.interface.get() };
    drop(scoped_sfs);

    let mut root_volume = open_root_volume(sfs).expect("Faled to open root volume.");
    let mut kernel_file = open_kernel_file(&mut root_volume).expect("Failed to read kernel file.");

    writeln!(system_table.stdout(), "Kernel file loaded.").unwrap();

    loop {}
}
