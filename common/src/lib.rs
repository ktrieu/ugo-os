#![no_std]

use core::{fmt::Debug, ops, slice};

// The end of the x86-64 low canonical addresses. This marks the end of userspace memory.
pub const UMEM_END: u64 = 0x0000_7FFF_FFFF_FFFF;
// The start of the x86-64 high canonical addresses. We'll be using this to indicate kernel memory.
pub const KMEM_START: u64 = 0xFFFF_8000_0000_0000;

// This is the default. If we have configurable page sizes later, it will be a huge success for this project.
pub const PAGE_SIZE: u64 = 4096;

// This is the number of bits that are allowed to be set in a physical address.
pub const PHYSADDR_SIZE: u8 = 52;

// This is the number of bits that are used in a virtual address. The upper bits must be sign-extended.
pub const VIRTADDR_SIZE: u8 = 48;

#[derive(Debug, Clone, Copy)]
pub enum RegionType {
    Usable,
    Allocated,
    Bootloader,
}

#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct MemRegion {
    pub start: u64,
    // End address, exclusive
    pub end: u64,
    pub ty: RegionType,
}

#[repr(C)]
#[derive(Clone, Copy)]
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
