use core::arch::asm;

use common::{KERNEL_START, PHYSMEM_START};
use uefi::table::boot::MemoryMap;

use crate::{
    addr::{PhysAddr, PhysFrame, VirtPage},
    frame::FrameAllocator,
    page::{IntermediatePageTable, PageMapLevel4, PageTableEntry},
    page::{PageMapLevel1, PageTable},
};

pub struct MappingFlags {
    exec: bool,
    write: bool,
}

impl MappingFlags {
    pub fn set_for_entry(&self, entry: &mut PageTableEntry) {
        entry.set_no_exec(!self.exec);
        entry.set_write(self.write);
    }

    pub fn new(exec: bool, write: bool) -> MappingFlags {
        MappingFlags { exec, write }
    }

    pub fn new_rw_data() -> MappingFlags {
        MappingFlags {
            exec: false,
            write: true,
        }
    }

    pub fn new_r_data() -> MappingFlags {
        MappingFlags {
            exec: false,
            write: false,
        }
    }

    pub fn new_code() -> MappingFlags {
        MappingFlags {
            exec: true,
            write: false,
        }
    }
}

fn map_page_entry(
    frame: PhysFrame,
    page: VirtPage,
    table: &mut PageMapLevel1,
    flags: MappingFlags,
) {
    let entry = table.get_entry_mut(page.base_addr());

    entry.set_addr(frame.base_addr());
    entry.set_present(true);

    flags.set_for_entry(entry);
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

    pub fn map_page(
        &mut self,
        frame: PhysFrame,
        page: VirtPage,
        allocator: &mut FrameAllocator,
        flags: MappingFlags,
    ) {
        let addr = page.base_addr();

        let level_3_map = self.level_4_map.get_mut_or_insert(addr, allocator);
        let level_2_map = level_3_map.get_mut_or_insert(addr, allocator);
        let level_1_map = level_2_map.get_mut_or_insert(addr, allocator);

        map_page_entry(frame, page, level_1_map, flags);
    }

    pub fn map_physical_memory(&mut self, memory_map: &MemoryMap, allocator: &mut FrameAllocator) {
        let highest_segment = memory_map
            .entries()
            .max_by_key(|descriptor| descriptor.phys_start)
            .expect("Memory map was empty!");

        let start_frame = PhysFrame::from_base_u64(0);
        let end_frame = PhysFrame::from_base_u64(highest_segment.phys_start)
            .add_frames(highest_segment.page_count);

        let start_page = start_frame.to_virt_page(PHYSMEM_START);
        let end_page = end_frame.to_virt_page(PHYSMEM_START);

        bootlog!(
            "Mapping all physical memory.\n{} - {}\n{} - {}",
            start_frame,
            end_frame,
            start_page,
            end_page
        );

        let frame_range = start_frame.range_inclusive(end_frame);
        let page_range = start_page.range_inclusive(end_page);

        for (frame, page) in frame_range.zip(page_range) {
            self.map_page(frame, page, allocator, MappingFlags::new_rw_data());
        }
    }

    // fn_ptr should be derived from a function, but I can't have a "pointer to any function"
    // as a type, AFAIK.
    pub fn identity_map_fn(&mut self, fn_ptr: *const (), allocator: &mut FrameAllocator) {
        let addr = fn_ptr as u64;

        let frame = PhysFrame::from_containing_u64(addr);
        let page = VirtPage::from_containing_u64(addr);

        bootlog!("Identity mapping {}", frame);

        self.map_page(frame, page, allocator, MappingFlags::new_code());
        // The function might lie on a page boundary, so map the next one too.
        bootlog!("Identity mapping {}", frame.next_frame());
        self.map_page(
            frame.next_frame(),
            page.next_page(),
            allocator,
            MappingFlags::new_code(),
        )
    }

    pub unsafe fn activate(&self) {
        asm!(
            "mov cr3, {addr}",
            addr = in(reg) self.level_4_phys_addr.as_u64()
        );
    }

    pub fn level_4_phys_addr(&self) -> PhysAddr {
        self.level_4_phys_addr
    }
}
