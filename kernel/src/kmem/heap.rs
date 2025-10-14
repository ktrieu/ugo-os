use core::{
    alloc::Layout,
    cmp::max,
    ptr::{self, null_mut, NonNull},
};

use common::addr::{align_up, is_aligned, Address, Page, VirtAddr, VirtPageRange};

#[derive(Debug)]
struct FreeSegment {
    prev: Option<NonNull<FreeSegment>>,
    next: Option<NonNull<FreeSegment>>,
    // Size is inclusive of this header, since once we allocate from this the header will
    // be overwritten.
    size: usize,
}

#[derive(Debug)]
struct AllocationHeader {
    // [start, end) of this allocation. start may not necessarily be equal to this
    // struct's offset in memory due to alignment.
    start: usize,
    end: usize,
}

const ALLOC_HEADER_ALIGN: usize = align_of::<AllocationHeader>();
const ALLOC_HEADER_SIZE: usize = size_of::<AllocationHeader>();

const FREE_HEADER_ALIGN: usize = align_of::<FreeSegment>();
const FREE_HEADER_SIZE: usize = align_of::<FreeSegment>();

fn offset_range_exclusive(start: *const FreeSegment, size: usize) -> (usize, usize) {
    let start = start as usize;

    (start, start + size)
}

fn vaddr_range_inclusive(start: *const FreeSegment, size: usize) -> (VirtAddr, VirtAddr) {
    let start = start as usize;
    let end = start + size - 1;

    (VirtAddr::new(start as u64), VirtAddr::new(end as u64))
}

pub struct KernelHeap {
    pages: VirtPageRange,
    free_head: Option<NonNull<FreeSegment>>,
}

#[derive(Debug)]
struct AllocResult {
    // Where to place the allocation header.
    header_start: usize,
    // The start of the usable space returned from this alloc.
    alloc_start: usize,
    // The end of the usable space returned from this alloc.
    alloc_end: usize,
    // The remaining space in this free segment. May be None if the remaining space is too small for a segment.
    remaining: Option<(usize, usize)>,
}

impl KernelHeap {
    // Safety: the range [dst, dst + size] must not be used or referenced.
    unsafe fn write_free_segment(&mut self, segment: FreeSegment, dst: *mut FreeSegment) {
        // Some sanity checks...
        assert!(is_aligned(dst as u64, align_of::<FreeSegment>() as u64));

        let (start, end) = vaddr_range_inclusive(dst, segment.size);
        assert!(self.pages.contains_addr(start));
        assert!(self.pages.contains_addr(end));

        kprintln!("Writing {:?} to {:?}", segment, dst);

        ptr::write(dst, segment);
    }

    // Safety: segment must point to a valid FreeSegment inside this heap.
    unsafe fn try_alloc_segment(
        segment: *const FreeSegment,
        size: usize,
        align: u64,
    ) -> Option<AllocResult> {
        // Hack - fix by creating a handle type for free segments from the heap.
        // This is safe because we asserted that the above pointer is valid.
        let free_size = unsafe { (*segment).size };
        let (free_start, free_end) = offset_range_exclusive(segment, free_size);

        // Our usable memory starts after allocating space for our allocation header.
        // Align up to ensure we have a start address that's compatible with the requested alignment.
        let alloc_start = align_up((free_start + ALLOC_HEADER_SIZE) as u64, align) as usize;
        // Usable memory is the start + size. Align up to ensure we always start our next free segment (or alloc header) at the correct alignment.
        let alloc_end = align_up((alloc_start + size) as u64, FREE_HEADER_ALIGN as u64) as usize;

        let header_start = alloc_start - size_of::<AllocationHeader>();
        // Double check that our header start is correctly aligned.
        assert!(is_aligned(
            header_start as u64,
            align_of::<AllocationHeader>() as u64
        ));

        let remaining_start = alloc_end;
        let remaining_end = free_end;
        if remaining_end < remaining_start {
            // Segment's too big. This won't work.
            return None;
        };

        let remaining = if remaining_end - remaining_start < FREE_HEADER_SIZE {
            // If there's not enough space for another free segment in this header, then the remaining space is None.
            None
        } else {
            // Otherwise, return the remaining range.
            Some((remaining_start, remaining_end))
        };

        Some(AllocResult {
            header_start,
            alloc_start,
            alloc_end,
            remaining,
        })
    }

    // Safety: pages must refer to memory that is not being used or referenced.
    pub unsafe fn new(pages: VirtPageRange) -> Self {
        let mut heap = Self {
            pages,
            free_head: None,
        };

        let size = pages.len_bytes() as usize;
        let initial_segment = FreeSegment {
            prev: None,
            next: None,
            size,
        };

        let head = pages.first().base_addr().as_u8_ptr_mut() as *mut FreeSegment;
        heap.write_free_segment(initial_segment, head);
        heap.free_head = Some(NonNull::new(head).expect("heap initial head should be non-null"));

        // try to allocate something?
        let result = Self::try_alloc_segment(heap.free_head.unwrap().as_ptr(), 16, 16);
        kprintln!("{:?}", result);

        heap
    }

    pub fn alloc(&mut self, layout: Layout) -> *mut u8 {
        // for each element in the list of free segments:
        // try to determine allocation bounds:
        // - align up to requested alignment
        // - add size of allocation
        // - see if there is room for another freesegment (we'll have to write it!)
        // grab free segment based on best fit
        // do bookkeeping, return pointer.
        null_mut()
    }

    pub fn free(&mut self, ptr: *mut u8, layout: Layout) {}
}
