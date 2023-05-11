use uefi::table::boot::{MemoryMap, MemoryType};

use crate::addr::{PhysAddr, PhysFrame};

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
            alloc_end: PhysFrame::from_base_u64(first_free.phys_start).add_frames(min_frames),
            next_frame: PhysFrame::from_base_u64(first_free.phys_start),
        }
    }

    pub fn alloc_frame(&mut self) -> PhysFrame {
        // We could return a Result I suppose, but this is basically unrecoverable.
        if self.next_frame.base_addr() >= self.alloc_end.base_addr() {
            panic!("Used all reserved boot physical memory!")
        }

        let ret = self.next_frame;
        self.next_frame = ret.next_frame();

        ret
    }

    pub fn alloc_start(&self) -> PhysAddr {
        self.alloc_start.base_addr()
    }

    pub fn alloc_end(&self) -> PhysAddr {
        self.alloc_end.base_addr()
    }
}
