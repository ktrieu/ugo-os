use common::PAGE_SIZE;
use uefi::table::boot::{MemoryDescriptor, MemoryType};

use crate::page::{PhysAddr, PhysFrame};

pub struct FrameAllocator<'a, I>
where
    I: ExactSizeIterator<Item = &'a MemoryDescriptor> + Clone,
{
    descriptors: I,
    current_descriptor: &'a MemoryDescriptor,

    // The first frame we've allocated, inclusive
    alloc_start: PhysFrame,
    // The next frame we're about to allocate
    next_frame: PhysFrame,
}

impl<'a, I> FrameAllocator<'a, I>
where
    I: ExactSizeIterator<Item = &'a MemoryDescriptor> + Clone,
{
    pub fn new(descriptors: I) -> Self {
        // Find the first free descriptor
        let mut descriptors = descriptors.clone();
        let first_free = descriptors
            .find(|descriptor| descriptor.ty == MemoryType::CONVENTIONAL)
            .expect("No free memory for frame allocator!");

        FrameAllocator {
            descriptors: descriptors,
            current_descriptor: first_free,
            alloc_start: PhysFrame::from_base_u64(first_free.phys_start),
            next_frame: PhysFrame::from_base_u64(first_free.phys_start),
        }
    }

    pub fn alloc_frame(&mut self) -> PhysFrame {
        let current_descriptor_end = PhysAddr::new(
            self.current_descriptor.phys_start + self.current_descriptor.page_count * PAGE_SIZE,
        );
        if self.next_frame.base_addr() > current_descriptor_end {
            self.current_descriptor = self
                .descriptors
                .find(|descriptor| descriptor.ty == MemoryType::CONVENTIONAL)
                .expect("No more usable memory descriptors for frame allocator!");
            self.next_frame = PhysFrame::from_base_u64(self.current_descriptor.phys_start);
        }

        let ret = self.next_frame;
        self.next_frame = ret.next_frame();

        ret
    }
}
