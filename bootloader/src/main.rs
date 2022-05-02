#![no_main]
#![no_std]
#![feature(abi_efiapi)]

use core::{fmt::Write, slice};

use uefi::{
    prelude::*,
    proto::media::{
        file::{Directory, File, FileAttribute, FileInfo, FileMode, FileType, RegularFile},
        fs::SimpleFileSystem,
    },
    table::boot::{AllocateType, MemoryType},
    CStr16,
};

fn open_root_volume(sfs: &mut SimpleFileSystem) -> Result<Directory, uefi::Error> {
    sfs.open_volume()
}

fn open_kernel_file(dir: &mut Directory) -> Result<RegularFile, uefi::Error> {
    // The open function only takes CStr16's, and converting it is sort of involved...
    let mut buf: [u16; 11] = [0; 11]; // 10 chars for the name, plus 1 for null terminator

    let handle = dir.open(
        CStr16::from_str_with_buf("ugo-os.elf", &mut buf).unwrap(),
        FileMode::Read,
        FileAttribute::VALID_ATTR,
    )?;

    match handle.into_type() {
        Ok(FileType::Regular(file)) => Ok(file),
        _ => Err(uefi::Error::new(Status::NOT_FOUND, ())),
    }
}

fn get_file_size(boot_services: &BootServices, file: &mut RegularFile) -> Result<u64, uefi::Error> {
    let mut buf = [0; 16];
    // Fetch with a zero size buffer first to get the actual size we need
    let requested_buffer_size = match file.get_info::<FileInfo>(&mut buf) {
        Err(uefi_err) => match uefi_err.data() {
            Some(buffer_size) => *buffer_size,
            // If there's no error, then I guess zero is the requested buffer size
            _ => 16,
        },
        _ => 16,
    };

    let buf = boot_services.allocate_pool(MemoryType::LOADER_DATA, requested_buffer_size)?;

    let file_size = match file
        .get_info::<FileInfo>(unsafe { slice::from_raw_parts_mut(buf, requested_buffer_size) })
    {
        Ok(file_info) => Ok(file_info.file_size()),
        Err(err) => Err(uefi::Error::new(err.status(), ())),
    };

    boot_services.free_pool(buf)?;

    return file_size;
}

fn read_file_data<'a>(
    boot_services: &BootServices,
    file: &'a mut RegularFile,
) -> Result<&'a [u8], uefi::Error> {
    let file_size: usize = get_file_size(boot_services, file)?
        .try_into()
        .expect("Kernel file larger than usize!");

    let file_buf =
        boot_services.allocate_pages(AllocateType::AnyPages, MemoryType::LOADER_DATA, file_size)?
            as *mut u8;

    // Weird: the error case for this isn't a uefi::Error, only an Option<usize>. Since we've already done
    // the file size checking earlier, we assume this always succeeds.
    file.read(unsafe { slice::from_raw_parts_mut(file_buf, file_size) })
        .unwrap();

    Ok(unsafe { slice::from_raw_parts(file_buf, file_size) })
}

fn locate_sfs<'a>(boot_services: &'a BootServices) -> Result<&mut SimpleFileSystem, uefi::Error> {
    boot_services
        .locate_protocol::<SimpleFileSystem>()
        .map(|protocol_ref| unsafe { &mut *protocol_ref.get() })
}

#[entry]
fn uefi_main(_handle: Handle, mut system_table: SystemTable<Boot>) -> Status {
    uefi_services::init(&mut system_table).unwrap();

    writeln!(system_table.stdout(), "Hello from ugoOS!!").unwrap();

    let sfs =
        locate_sfs(system_table.boot_services()).expect("Failed to locate filesystem protocol.");

    let mut root_volume = open_root_volume(sfs).expect("Failed to open root volume.");
    let mut kernel_file = open_kernel_file(&mut root_volume).expect("Failed to open kernel file.");
    let file = read_file_data(system_table.boot_services(), &mut kernel_file)
        .expect("Failed to read kernel file.");

    writeln!(
        system_table.stdout(),
        "Kernel file loaded. File size: {}. ELF header: {:x?}.",
        file.len(),
        &file[0..4]
    )
    .unwrap();

    loop {}
}
