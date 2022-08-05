use core::slice;

use alloc::vec::Vec;
use uefi::{
    prelude::BootServices,
    table::boot::{MemoryDescriptor, MemoryMapSize, MemoryType},
};

use common::{MemRegion, RegionType, PAGE_SIZE};

fn descriptor_type_to_region_type(ty: MemoryType) -> RegionType {
    match ty {
        MemoryType::CONVENTIONAL => RegionType::Usable,
        _ => RegionType::Allocated,
    }
}

fn descriptor_to_region(descriptor: &MemoryDescriptor) -> MemRegion {
    MemRegion {
        start: descriptor.phys_start,
        end: descriptor.phys_start + (descriptor.page_count * PAGE_SIZE),
        ty: descriptor_type_to_region_type(descriptor.ty),
    }
}

pub fn get_memory_map(boot_services: &BootServices) -> Result<Vec<MemRegion>, uefi::Error> {
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

    // Preallocate a vector so its allocation is captured in the memory map
    let mut regions: Vec<MemRegion> = Vec::with_capacity(map_size);

    let (_, descriptors) = boot_services.memory_map(buffer)?;

    regions.extend(descriptors.map(descriptor_to_region));

    Ok(regions)
}
