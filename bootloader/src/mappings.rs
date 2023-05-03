use crate::{
    addr::{PhysAddr, PhysFrame, VirtPage},
    frame::FrameAllocator,
    page::{IntermediatePageTable, PageMapLevel4},
    page::{PageMapLevel1, PageTable},
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

    fn map_page_entry(frame: PhysFrame, page: VirtPage, table: &mut PageMapLevel1) {
        let entry = table.get_entry_mut(page.base_addr());

        entry.set_addr(frame.base_addr());
    }

    pub fn map_page(&mut self, frame: PhysFrame, page: VirtPage, allocator: &mut FrameAllocator) {
        let addr = page.base_addr();

        let level_3_map = self.level_4_map.get_mut_or_insert(addr, allocator);
        let level_2_map = level_3_map.get_mut_or_insert(addr, allocator);
        let level_1_map = level_2_map.get_mut_or_insert(addr, allocator);
    }
}
