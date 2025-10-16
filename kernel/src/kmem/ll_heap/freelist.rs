use core::fmt::{Debug, Formatter, Result};
use core::num::NonZero;
use core::ops::Sub;
use core::ptr;

use common::addr::{Address, Page, VirtPageRange};

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
struct HeapOffset(NonZero<usize>);

impl HeapOffset {
    fn as_mut_entry_ptr(&self) -> *mut Entry {
        self.0.get() as *mut Entry
    }
}

impl Sub for HeapOffset {
    type Output = usize;

    fn sub(self, rhs: Self) -> Self::Output {
        self.0.get() - rhs.0.get()
    }
}

pub struct FreeList {
    pages: VirtPageRange,
    head: Option<HeapOffset>,
}

struct Entry {
    prev: Option<HeapOffset>,
    next: Option<HeapOffset>,
    size: usize,
}

// A block of free memory in the freelist. start always corresponds to
// the base address of an Entry struct.
struct FreeBlock {
    start: HeapOffset,
    end: HeapOffset,
}

impl FreeBlock {
    pub fn size(&self) -> usize {
        self.end - self.start
    }
}

impl FreeList {
    // Safety: pages has to be valid for writes and unused.
    pub unsafe fn new(pages: VirtPageRange) -> Self {
        let mut free_list = FreeList {
            pages: pages,
            head: None,
        };
        // Initialize the heap with one giant free block.
        let init_block = FreeBlock {
            start: free_list.offset(pages.first().base_addr().as_u64() as usize),
            end: free_list.offset(pages.end().base_addr().as_u64() as usize),
        };

        // Safety: The whole page range is clear and valid at this point, so we can write the one
        // big segment.
        unsafe {
            let ptr = init_block.start.as_mut_entry_ptr();

            let entry = Entry {
                prev: None,
                next: None,
                size: init_block.size(),
            };

            ptr::write(ptr, entry)
        };

        free_list.head = Some(init_block.start);

        free_list
    }

    pub fn offset(&self, o: usize) -> HeapOffset {
        // Encapsulating this in a function allows us to enforce that the offsets are correct.
        assert!(o >= self.pages.first().base_u64() as usize);
        assert!(o < self.pages.end().base_u64() as usize);

        HeapOffset(NonZero::new(o).expect("heap offset should be non-zero"))
    }
}
