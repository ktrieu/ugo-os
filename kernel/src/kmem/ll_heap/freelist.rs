use core::fmt::{Debug, Formatter, Result};
use core::num::NonZero;
use core::ops::{Add, Sub};
use core::ptr;

use common::addr::{Address, Page, VirtPageRange};

use crate::kmem::ll_heap::is_aligned_us;

#[derive(Clone, Copy, PartialEq, Eq)]
pub struct HeapOffset(NonZero<usize>);

impl HeapOffset {
    pub fn as_ptr_mut<T>(&self) -> *mut T {
        self.0.get() as *mut T
    }

    pub fn as_ptr<T>(&self) -> *const T {
        self.0.get() as *const T
    }

    // Same invariants as ptr::write apply.
    pub unsafe fn write<T>(&self, value: T) {
        let ptr = self.as_ptr_mut::<T>();
        ptr::write(ptr, value);
    }
}

impl Sub for HeapOffset {
    type Output = usize;

    fn sub(self, rhs: Self) -> Self::Output {
        self.0.get() - rhs.0.get()
    }
}

impl From<HeapOffset> for usize {
    fn from(value: HeapOffset) -> Self {
        value.0.get()
    }
}

impl Debug for HeapOffset {
    // This is morally a pointer so print it that way.
    fn fmt(&self, f: &mut Formatter<'_>) -> Result {
        write!(f, "{:x}", self.0.get())
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

pub const FREELIST_ENTRY_ALIGN: usize = align_of::<Entry>();
pub const FREELIST_ENTRY_SIZE: usize = size_of::<Entry>();

// A block of free memory in the freelist. start always corresponds to
// the base address of an Entry struct.
#[derive(Debug)]
pub struct FreeBlock {
    start: HeapOffset,
    end: HeapOffset,
}

impl FreeBlock {
    pub fn size(&self) -> usize {
        self.end - self.start
    }

    pub fn start(&self) -> HeapOffset {
        self.start
    }

    pub fn end(&self) -> HeapOffset {
        self.end
    }
}
// A block of allocated memory parceled out from a Freeblock
#[derive(Debug)]
pub struct AllocBlock {
    start: HeapOffset,
    end: HeapOffset,
}

impl AllocBlock {
    pub fn size(&self) -> usize {
        self.end - self.start
    }

    pub fn start(&self) -> HeapOffset {
        self.start
    }

    pub fn end(&self) -> HeapOffset {
        self.end
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
            let entry = Entry {
                prev: None,
                next: None,
                size: init_block.size(),
            };

            init_block.start.write(entry);
        };

        free_list.head = Some(init_block.start);

        free_list
    }

    pub fn offset(&self, o: usize) -> HeapOffset {
        // Encapsulating this in a function allows us to enforce that the offsets are inside this range.
        assert!(o >= self.pages.first().base_u64() as usize);
        assert!(o <= self.pages.end().base_u64() as usize);

        HeapOffset(NonZero::new(o).expect("heap offset should be non-zero"))
    }

    // Safety: offset must point to a valid Entry struct.
    unsafe fn entry_from_offset_mut(&mut self, offset: HeapOffset) -> &mut Entry {
        let ptr = offset.as_ptr_mut::<Entry>();

        ptr.as_mut().expect("offset must not be null")
    }

    unsafe fn entry_from_offset(&self, offset: HeapOffset) -> &Entry {
        let ptr = offset.as_ptr::<Entry>();

        ptr.as_ref().expect("offset must not be null")
    }

    fn block_entry_mut(&mut self, block: &FreeBlock) -> &mut Entry {
        // FreeBlock.start is always that block's Entry address.
        unsafe { self.entry_from_offset_mut(block.start) }
    }

    // Resizes the given block by removing len bytes from the beginning.
    pub fn resize_block(&mut self, block: FreeBlock, len: usize) -> AllocBlock {
        assert!(len < block.size());

        let (old_prev, old_next) = {
            let entry = self.block_entry_mut(&block);
            (entry.prev, entry.next)
        };

        let new_block = FreeBlock {
            start: self.offset(usize::from(block.start()) + len),
            end: block.end,
        };

        assert!(is_aligned_us(
            usize::from(block.start()),
            FREELIST_ENTRY_ALIGN
        ));

        let new_entry = Entry {
            prev: old_prev,
            next: old_next,
            size: new_block.size(),
        };

        unsafe {
            // The new start is in an existing FreeBlock, so it's unused and safe to write to.
            new_block.start.write(new_entry);
        };

        // Fix up our pointers.
        if let Some(prev) = old_prev {
            // prev must be valid, comes from a valid Entry
            let prev_entry = unsafe { self.entry_from_offset_mut(prev) };
            prev_entry.next = Some(new_block.start);
        }

        if let Some(next) = old_next {
            // next must be valid, comes from a valid Entry
            let next_entry = unsafe { self.entry_from_offset_mut(next) };
            next_entry.prev = Some(new_block.start);
        }

        // And don't forget to fix up our head pointer.
        if self.head.is_some_and(|h| h == block.start) {
            self.head = Some(new_block.start);
        }

        let alloc_block = AllocBlock {
            start: block.start,
            end: self.offset(usize::from(block.start) + len),
        };

        alloc_block
    }

    pub fn head(&self) -> Option<FreeBlock> {
        unsafe {
            self.head.map(|o| {
                let entry = self.entry_from_offset(o);

                FreeBlock {
                    start: o,
                    end: self.offset(usize::from(o) + entry.size),
                }
            })
        }
    }
}
