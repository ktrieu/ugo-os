use common::PAGE_SIZE;

// Allocates virtual addresses, one page at a time.
// We probably aren't going to ever free anything, so this is just a simple bump allocator.
pub struct VirtualAllocator {
    next_addr: u64,
}

#[derive(Debug)]
pub enum VirtualAllocError {
    Exhausted,
}

impl VirtualAllocator {
    pub fn new(start_addr: u64) -> VirtualAllocator {
        VirtualAllocator {
            next_addr: start_addr,
        }
    }

    pub fn allocate(&mut self, num_pages: u64) -> Result<u64, VirtualAllocError> {
        let end_addr = self.next_addr.checked_add(num_pages * PAGE_SIZE);
        match end_addr {
            Some(end_addr) => {
                let alloc_start = self.next_addr;
                self.next_addr = end_addr;
                Ok(alloc_start)
            }
            None => Err(VirtualAllocError::Exhausted),
        }
    }
}
