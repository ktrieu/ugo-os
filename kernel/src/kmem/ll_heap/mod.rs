use core::alloc::Layout;

use common::addr::{align_up, is_aligned, VirtPageRange};

use crate::kmem::ll_heap::freelist::{
    FreeBlock, FreeList, HeapOffset, FREELIST_ENTRY_ALIGN, FREELIST_ENTRY_SIZE,
};

pub mod freelist;

#[derive(Debug)]
struct AllocHeader {
    // [start, end) of this allocation. start may not necessarily be equal to this
    // struct's offset in memory due to alignment.
    start: HeapOffset,
    end: HeapOffset,
}

const ALLOC_HEADER_ALIGN: usize = align_of::<AllocHeader>();
const ALLOC_HEADER_SIZE: usize = size_of::<AllocHeader>();

fn align_up_us(addr: usize, align: usize) -> usize {
    // let's just assume we're on x64 for now.
    align_up(addr as u64, align as u64) as usize
}
fn is_aligned_us(addr: usize, align: usize) -> bool {
    is_aligned(addr as u64, align as u64)
}

pub struct KernelHeap {
    pages: VirtPageRange,
    free_list: FreeList,
}

#[derive(Debug)]
struct AllocResult {
    // Where to place the allocation header.
    header_start: HeapOffset,
    // The start of the allocated memory - this is the final value that gets returned from alloc.
    alloc_start: HeapOffset,
    // The start of the space taken by this allocation - including header and padding.
    used_start: HeapOffset,
    // The end of the space taken by this allocation - including header and padding.
    used_end: HeapOffset,
    // The remaining space in the free header. May be None if the remaining space is too small for a header.
    remaining: Option<(HeapOffset, HeapOffset)>,
}

impl KernelHeap {
    fn try_alloc_from_free_block(
        &self,
        block: &FreeBlock,
        size: usize,
        align: usize,
    ) -> Option<AllocResult> {
        // Our usable memory starts after allocating space for our allocation header.
        // Align up to ensure we have a start address that's compatible with the requested alignment.
        let alloc_start = align_up_us(usize::from(block.start()) + ALLOC_HEADER_SIZE, align);

        // Usable memory is the start + size. Align up to ensure we always start our next free header (or alloc header) at the correct alignment.
        let alloc_end = align_up_us(alloc_start + size, FREELIST_ENTRY_ALIGN);

        let header_start = alloc_start - size_of::<AllocHeader>();
        // Double check that our header start is correctly aligned.
        assert!(is_aligned_us(header_start, ALLOC_HEADER_ALIGN));

        let remaining_start = alloc_end;
        let remaining_end = block.end().into();
        if remaining_end < remaining_start {
            // Allocation's too big. This won't work.
            return None;
        };

        let remaining = if remaining_end - remaining_start < FREELIST_ENTRY_SIZE {
            // If there's not enough space for another free header in this block, then the remaining space is None.
            None
        } else {
            // Otherwise, return the remaining range.
            Some((
                self.free_list.offset(remaining_start),
                self.free_list.offset(remaining_end),
            ))
        };

        // The start of the used space is just the start of this block.
        let used_start = block.start().into();
        // If we have some space for another header, the used_end is the end of the allocation space.
        // Otherwise, the used_end is the end of this block.
        let used_end = match remaining {
            Some(_) => alloc_end,
            None => block.end().into(),
        };

        Some(AllocResult {
            header_start: self.free_list.offset(header_start),
            used_start: self.free_list.offset(used_start),
            alloc_start: self.free_list.offset(alloc_start),
            used_end: self.free_list.offset(used_end),
            remaining,
        })
    }

    // Safety: pages must refer to memory that is not being used or referenced.
    pub unsafe fn new(pages: VirtPageRange) -> Self {
        kprintln!("Initializing kernel heap: {}", pages);
        Self {
            pages,
            free_list: FreeList::new(pages),
        }
    }

    pub fn alloc(&mut self, layout: Layout) -> *mut u8 {
        // For now, just grab from the head.
        let head = self.free_list.iter().next().expect("head must exist");

        let details = self
            .try_alloc_from_free_block(&head, layout.size(), layout.align())
            .expect("try_alloc should succeed");

        let alloc_block = self
            .free_list
            .resize_block(head, details.used_end - details.used_start);

        // Sanity checks...
        assert!(alloc_block.start() == details.used_start);
        assert!(alloc_block.end() == details.used_end);

        let header = AllocHeader {
            start: details.used_start,
            end: details.used_end,
        };

        assert!(is_aligned_us(
            details.header_start.into(),
            ALLOC_HEADER_ALIGN
        ));
        assert!(usize::from(details.header_start) + ALLOC_HEADER_SIZE < usize::from(header.end));
        // The header should be right before the pointer we return, or we'll never be able to recover it in `free`.
        assert!(
            usize::from(details.header_start) + ALLOC_HEADER_SIZE
                == usize::from(details.alloc_start)
        );

        // AllocHeader represents memory
        // that came from a free block, so it's free for writing.
        unsafe {
            details.header_start.write(header);
        }

        details.alloc_start.as_ptr_mut::<u8>()
    }

    pub fn free(&mut self, ptr: *mut u8, layout: Layout) {}
}
