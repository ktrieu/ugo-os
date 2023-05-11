use core::slice;

use uefi::{
    prelude::{cstr16, BootServices},
    proto::media::{
        file::{Directory, File, FileAttribute, FileInfo, FileMode, FileType, RegularFile},
        fs::SimpleFileSystem,
    },
    table::boot::{AllocateType, MemoryType, ScopedProtocol},
    CStr16, Status,
};

pub fn open_root_volume(
    sfs: &mut ScopedProtocol<SimpleFileSystem>,
) -> Result<Directory, uefi::Error> {
    sfs.open_volume()
}

const KERNEL_FILENAME: &CStr16 = cstr16!("ugo-os.elf");

pub fn open_kernel_file(dir: &mut Directory) -> Result<RegularFile, uefi::Error> {
    let handle = dir.open(KERNEL_FILENAME, FileMode::Read, FileAttribute::VALID_ATTR)?;

    match handle.into_type() {
        Ok(FileType::Regular(file)) => Ok(file),
        _ => Err(uefi::Error::new(Status::NOT_FOUND, ())),
    }
}

fn get_file_size(boot_services: &BootServices, file: &mut RegularFile) -> Result<u64, uefi::Error> {
    let mut buf = [0; 0];
    // Fetch with a zero size buffer first to get the actual size we need
    let requested_buffer_size = match file.get_info::<FileInfo>(&mut buf) {
        Err(uefi_err) => match uefi_err.data() {
            Some(buffer_size) => *buffer_size,
            // If there's no error, then I guess zero is the requested buffer size
            _ => 0,
        },
        _ => 0,
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

pub fn read_file_data(
    boot_services: &BootServices,
    file: &mut RegularFile,
) -> Result<&'static [u8], uefi::Error> {
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

pub fn locate_sfs<'a>(
    boot_services: &'a BootServices,
) -> Result<ScopedProtocol<SimpleFileSystem>, uefi::Error> {
    let handle = boot_services.get_handle_for_protocol::<SimpleFileSystem>()?;
    boot_services.open_protocol_exclusive::<SimpleFileSystem>(handle)
}
