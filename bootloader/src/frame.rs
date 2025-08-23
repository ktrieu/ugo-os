use common::PAGE_SIZE;
use uefi::table::boot::{MemoryMap, MemoryType};

use common::addr::{Address, Page, PageRange, PhysAddr, PhysFrame};

pub struct FrameAllocator {
    // The first frame we've allocated, inclusive
    alloc_start: PhysFrame,
    // The end of our memory space, exclusive
    alloc_end: PhysFrame,
    // The next frame we're about to allocate
    next_frame: PhysFrame,
}

impl FrameAllocator {
    pub fn new(memory_map: &MemoryMap, min_frames: u64) -> Self {
        // Find the first free descriptor big enough
        let first_free = memory_map
            .entries()
            .find(|descriptor| {
                descriptor.ty == MemoryType::CONVENTIONAL && descriptor.page_count >= min_frames
            })
            .expect("Could not find large enough descriptor for frame allocator!");

        FrameAllocator {
            alloc_start: PhysFrame::from_base_u64(first_free.phys_start),
            alloc_end: PhysFrame::from_base_u64(first_free.phys_start).increment(min_frames),
            next_frame: PhysFrame::from_base_u64(first_free.phys_start),
        }
    }

    pub fn alloc_frame(&mut self) -> PhysFrame {
        // We could return a Result I suppose, but this is basically unrecoverable.
        if self.next_frame.base_addr() >= self.alloc_end.base_addr() {
            panic!("Used all reserved boot physical memory!")
        }

        let ret = self.next_frame;
        self.next_frame = ret.next();

        ret
    }

    pub fn alloc_frame_range(&mut self, len: u64) -> PageRange<PhysFrame> {
        let start = self.next_frame();
        let range = PhysFrame::range_length(start, len);

        for _ in 0..len {
            self.alloc_frame();
        }

        // Make sure the range we're returning and our internal allocation state matches.
        assert!(self.next_frame == range.last().next());

        range
    }

    pub fn alloc_start(&self) -> PhysAddr {
        self.alloc_start.base_addr()
    }

    pub fn alloc_end(&self) -> PhysAddr {
        self.alloc_end.base_addr()
    }

    pub fn reserved_range(&self) -> PageRange<PhysFrame> {
        PhysFrame::range_exclusive(self.alloc_start, self.alloc_end)
    }

    pub fn used_range(&self) -> PageRange<PhysFrame> {
        PhysFrame::range_length(self.alloc_start, self.frames_allocated())
    }

    pub fn frames_allocated(&self) -> u64 {
        (self.next_frame.base_addr().as_u64() - self.alloc_start.base_addr().as_u64()) / PAGE_SIZE
    }

    pub fn next_frame(&self) -> PhysFrame {
        self.next_frame
    }
}
