use uefi::table::boot::MemoryDescriptor;

use crate::{
    addr::{PhysAddr, PhysFrame, VirtPage},
    frame::FrameAllocator,
    page::PageMapLevel4,
    page::PageTable,
};

pub struct Mappings<'a> {
    level_4_map: &'a mut PageMapLevel4,
    level_4_phys_addr: PhysAddr,
}

impl<'a> Mappings<'a> {
    pub fn new<I>(allocator: &mut FrameAllocator) -> Self {
        let (level_4_map, level_4_phys_addr) = PageMapLevel4::alloc_new(allocator);
        Mappings {
            level_4_map,
            level_4_phys_addr,
        }
    }

    pub fn map_page<I>(frame: PhysFrame, page: VirtPage, allocator: &mut FrameAllocator) {}
}
