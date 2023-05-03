use common::{PAGE_SIZE, PHYSMEM_START};
use uefi::table::boot::MemoryDescriptor;

use crate::{
    addr::{PhysAddr, PhysFrame, VirtAddr, VirtPage},
    frame::FrameAllocator,
    page::{IntermediatePageTable, PageMapLevel4},
    page::{PageMapLevel1, PageTable},
};

fn map_page_entry(frame: PhysFrame, page: VirtPage, table: &mut PageMapLevel1) {
    let entry = table.get_entry_mut(page.base_addr());

    entry.set_addr(frame.base_addr());
}

pub struct Mappings<'a> {
    level_4_map: &'a mut PageMapLevel4,
    level_4_phys_addr: PhysAddr,
}

impl<'a> Mappings<'a> {
    pub fn new(allocator: &mut FrameAllocator) -> Self {
        let (level_4_map, level_4_phys_addr) = PageMapLevel4::alloc_new(allocator);
        Mappings {
            level_4_map,
            level_4_phys_addr,
        }
    }

    pub fn map_page(&mut self, frame: PhysFrame, page: VirtPage, allocator: &mut FrameAllocator) {
        let addr = page.base_addr();

        let level_3_map = self.level_4_map.get_mut_or_insert(addr, allocator);
        let level_2_map = level_3_map.get_mut_or_insert(addr, allocator);
        let level_1_map = level_2_map.get_mut_or_insert(addr, allocator);

        map_page_entry(frame, page, level_1_map);
    }

    pub fn map_physical_memory<'b, D>(&mut self, descriptors: D, allocator: &mut FrameAllocator)
    where
        D: ExactSizeIterator<Item = &'b MemoryDescriptor> + Clone,
    {
        let highest_segment = descriptors
            .max_by_key(|descriptor| descriptor.phys_start)
            .expect("Memory map was empty!");

        let end_frame = PhysFrame::from_base_u64(highest_segment.phys_start)
            .end_of_range_exclusive(highest_segment.page_count);

        bootlog!(
            "Mapping all physical memory.\nP: {:#016x} - {:#016x}\nV: {:#016x} - {:#016x}",
            0,
            end_frame.base_addr().as_u64(),
            PHYSMEM_START,
            PHYSMEM_START + end_frame.base_addr().as_u64()
        );

        let mut frame = PhysFrame::from_base_u64(0);
        let mut page = VirtPage::from_base_addr(VirtAddr::new(PHYSMEM_START));

        while frame.base_addr() < end_frame.base_addr() {
            self.map_page(frame, page, allocator);
            frame = frame.next_frame();
            page = page.next_page();
        }
    }
}
