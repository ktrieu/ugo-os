#![no_std]

use core::{ops, slice};

pub enum RegionType {
    Usable,
    Allocated,
    Bootloader,
}

#[repr(C)]
pub struct MemRegion {
    pub start: u64,
    // End address, exclusive
    pub end: u64,
    pub ty: RegionType,
}

#[repr(C)]
pub struct MemRegions {
    pub ptr: *mut MemRegion,
    pub len: usize,
}

impl ops::Deref for MemRegions {
    type Target = [MemRegion];

    fn deref(&self) -> &[MemRegion] {
        unsafe { slice::from_raw_parts(self.ptr, self.len) }
    }
}

impl ops::DerefMut for MemRegions {
    fn deref_mut(&mut self) -> &mut [MemRegion] {
        unsafe { slice::from_raw_parts_mut(self.ptr, self.len) }
    }
}

#[repr(C)]
pub struct BootInfo {
    mem_regions: MemRegion,
}
