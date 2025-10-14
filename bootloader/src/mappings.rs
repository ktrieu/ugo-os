use common::{
    addr::{Page, PageRange, PhysAddr, PhysFrame, VirtPage},
    page::{IntermediatePageTable, PageTable, PageTableEntry},
    HUGE_PAGE_SIZE_BYTES, HUGE_PAGE_SIZE_PAGES,
};
use uefi::table::boot::MemoryMap;

use crate::{
    frame::FrameAllocator,
    page::{BootPageMapLevel1, BootPageMapLevel4},
};

#[derive(Clone, Copy)]
pub struct MappingFlags {
    exec: bool,
    write: bool,
    present: bool,
}

impl MappingFlags {
    pub fn set_for_entry(&self, entry: &mut PageTableEntry) {
        entry.set_no_exec(!self.exec);
        entry.set_write(self.write);
        entry.set_present(self.present);
    }

    pub fn new(exec: bool, write: bool, present: bool) -> MappingFlags {
        MappingFlags {
            exec,
            write,
            present,
        }
    }

    pub fn new_rw_data() -> MappingFlags {
        MappingFlags {
            exec: false,
            write: true,
            present: true,
        }
    }

    pub fn new_code() -> MappingFlags {
        MappingFlags {
            exec: true,
            write: false,
            present: true,
        }
    }

    pub fn new_guard() -> MappingFlags {
        MappingFlags {
            exec: false,
            write: false,
            present: false,
        }
    }
}

fn map_page_entry(
    frame: PhysFrame,
    page: VirtPage,
    table: &mut BootPageMapLevel1,
    flags: MappingFlags,
) {
    let entry = table.get_entry_mut(page.base_addr());

    entry.set_addr(frame.base_addr());

    flags.set_for_entry(entry);
}

pub struct Mappings<'a> {
    level_4_map: &'a mut BootPageMapLevel4,
    level_4_phys_addr: PhysAddr,
}

impl<'a> Mappings<'a> {
    pub fn new(allocator: &mut FrameAllocator) -> Self {
        let (level_4_map, level_4_phys_addr) = BootPageMapLevel4::alloc_new(allocator);
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

    pub fn map_page_range(
        &mut self,
        frames: PageRange<PhysFrame>,
        pages: PageRange<VirtPage>,
        allocator: &mut FrameAllocator,
        flags: MappingFlags,
    ) {
        assert!(frames.len() == pages.len());

        for (frame, page) in frames.iter().zip(pages.iter()) {
            self.map_page(frame, page, allocator, flags);
        }
    }

    pub fn alloc_and_map_range(
        &mut self,
        pages: PageRange<VirtPage>,
        allocator: &mut FrameAllocator,
        flags: MappingFlags,
    ) -> PageRange<PhysFrame> {
        let frames = allocator.alloc_frame_range(pages.len());

        self.map_page_range(frames, pages, allocator, flags);

        frames
    }

    fn direct_map_range(
        &mut self,
        frame_range: PageRange<PhysFrame>,
        allocator: &mut FrameAllocator,
    ) {
        let start_page = frame_range.first().as_direct_mapped();
        let end_page = frame_range.end().as_direct_mapped();

        let page_range = VirtPage::range_exclusive(start_page, end_page);

        bootlog!("Direct mapping range:\n {} -> {}", frame_range, page_range);

        self.map_page_range(
            frame_range,
            page_range,
            allocator,
            MappingFlags::new_rw_data(),
        );
    }

    fn direct_map_huge_page(
        &mut self,
        frame_range: PageRange<PhysFrame>,
        allocator: &mut FrameAllocator,
    ) {
        let start_page = frame_range.first().as_direct_mapped();
        let end_page = frame_range.end().as_direct_mapped();

        let page_range = VirtPage::range_exclusive(start_page, end_page);

        // Make sure everything is aligned correctly.
        assert!(frame_range.first().base_u64() % HUGE_PAGE_SIZE_BYTES == 0);
        assert!(frame_range.end().base_u64() % HUGE_PAGE_SIZE_BYTES == 0);

        assert!(page_range.first().base_u64() % HUGE_PAGE_SIZE_BYTES == 0);
        assert!(page_range.end().base_u64() % HUGE_PAGE_SIZE_BYTES == 0);

        bootlog!(
            "Direct mapping with huge pages: {} - {}",
            frame_range,
            page_range
        );

        let frame_iter = frame_range.iter().step_by(HUGE_PAGE_SIZE_PAGES as usize);
        let page_iter = page_range.iter().step_by(HUGE_PAGE_SIZE_PAGES as usize);

        for (frame, page) in frame_iter.zip(page_iter) {
            bootlog!("Mapping 1GB page {} - {}", frame, page);
            let level_3_map = self
                .level_4_map
                .get_mut_or_insert(page.base_addr(), allocator);

            let new_entry = level_3_map.get_entry_mut(page.base_addr());

            new_entry.set_no_exec(true);
            new_entry.set_write(true);
            new_entry.set_present(true);
            new_entry.set_page_size(true);
            new_entry.set_addr(frame.base_addr());
        }
    }

    pub fn map_physical_memory(&mut self, memory_map: &MemoryMap, allocator: &mut FrameAllocator) {
        let highest_segment = memory_map
            .entries()
            // On my laptop, the UEFI memory map includes a really high physical segment at 0xFD00000000.
            // The loader maps from 0 -> highest known memory segment, so we map all memory from
            // 0x0 -> 0xFD00000000 which takes a thousand years.
            // We should be smarter and only map ranges mentioned in the memory map - but for now just hack around this.
            .filter(|d| d.phys_start != 0xFD00000000)
            .max_by_key(|descriptor| descriptor.phys_start)
            .expect("Memory map was empty!");

        let start_frame = PhysFrame::from_base_u64(0);
        let last_frame = PhysFrame::from_base_u64(highest_segment.phys_start)
            .increment(highest_segment.page_count);
        let frame_range = PhysFrame::range_exclusive(start_frame, last_frame);

        let (start, middle, end) = frame_range.aligned_range(HUGE_PAGE_SIZE_PAGES);

        if let Some(start) = start {
            self.direct_map_range(start, allocator);
        };

        if let Some(middle) = middle {
            self.direct_map_huge_page(middle, allocator);
        };

        if let Some(end) = end {
            self.direct_map_range(end, allocator);
        };
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
        bootlog!("Identity mapping {}", frame.next());
        self.map_page(
            frame.next(),
            page.next(),
            allocator,
            MappingFlags::new_code(),
        )
    }

    pub fn level_4_phys_addr(&self) -> PhysAddr {
        self.level_4_phys_addr
    }
}
