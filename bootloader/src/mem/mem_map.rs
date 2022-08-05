use core::slice;

use alloc::vec::Vec;
use uefi::{
    prelude::BootServices,
    table::boot::{MemoryDescriptor, MemoryMapSize, MemoryType},
};

pub fn get_memory_map(boot_services: &BootServices) -> Result<Vec<MemoryDescriptor>, uefi::Error> {
    let MemoryMapSize {
        entry_size,
        mut map_size,
    } = boot_services.memory_map_size();
    // Allocating memory might add a few descriptors, so just to be safe, reserve a few more
    map_size += 2;

    let buffer_size = entry_size * map_size;

    let buffer = unsafe {
        slice::from_raw_parts_mut(
            boot_services.allocate_pool(MemoryType::LOADER_DATA, buffer_size)?,
            buffer_size,
        )
    };

    let (_, descriptors) = boot_services.memory_map(buffer)?;

    Ok(descriptors.copied().collect())
}
