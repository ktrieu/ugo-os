use common::PAGE_SIZE;
use uefi::table::boot::{MemoryDescriptor, MemoryType};

pub struct FrameAllocator<'a, I: ExactSizeIterator<Item = &'a MemoryDescriptor> + Clone> {
    descriptors: I,
    current_descriptor: Option<&'a MemoryDescriptor>,
    current_addr: u64,
}

#[derive(Debug)]
pub enum FrameAllocatorError {
    NoMoreDescriptors,
}

impl<'a, I: ExactSizeIterator<Item = &'a MemoryDescriptor> + Clone> FrameAllocator<'a, I> {
    pub fn new(descriptors: I) -> Self {
        FrameAllocator {
            descriptors: descriptors,
            current_descriptor: None,
            // Start at 0x1000 to skip address 0, since we can't use it.
            current_addr: 0x1000,
        }
    }

    // Can we still use the current memory descriptor for allocating pages?
    fn can_use_current_descriptor(&self, pages: u64) -> bool {
        let alloc_size = pages * PAGE_SIZE;
        match self.current_descriptor {
            Some(d) => {
                let end = d.phys_start + (d.page_count * PAGE_SIZE);
                // Can't use a region that isn't free
                d.ty != MemoryType::CONVENTIONAL
                    // Or if we're past the end
                    || end < self.current_addr
                    // Or if we don't have space for the allocation requested
                    || end - self.current_addr < alloc_size
            }
            // Can't use a descriptor that doesn't exist
            None => true,
        }
    }

    pub fn allocate(&mut self, pages: u64) -> Result<u64, FrameAllocatorError> {
        while !self.can_use_current_descriptor(pages) {
            let next_descriptor = self
                .descriptors
                .next()
                .ok_or(FrameAllocatorError::NoMoreDescriptors)?;
            self.current_descriptor = Some(next_descriptor);
            self.current_addr = next_descriptor.phys_start;
        }

        let alloc_start = self.current_addr;
        self.current_addr += PAGE_SIZE;

        Ok(alloc_start)
    }
}
