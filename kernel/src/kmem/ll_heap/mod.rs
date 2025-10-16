use core::{
    alloc::Layout,
    cmp::max,
    ptr::{self, null, null_mut, NonNull},
};

use common::addr::{align_up, is_aligned, Address, Page, VirtAddr, VirtPageRange};

pub mod freelist;

#[derive(Debug)]
struct FreeHeader {
    prev: Option<NonNull<FreeHeader>>,
    next: Option<NonNull<FreeHeader>>,
    // Size is inclusive of this header, since once we allocate from this the header will
    // be overwritten.
    size: usize,
}

#[derive(Debug)]
struct AllocHeader {
    // [start, end) of this allocation. start may not necessarily be equal to this
    // struct's offset in memory due to alignment.
    start: usize,
    end: usize,
}

const ALLOC_HEADER_ALIGN: usize = align_of::<AllocHeader>();
const ALLOC_HEADER_SIZE: usize = size_of::<AllocHeader>();

const FREE_HEADER_ALIGN: usize = align_of::<FreeHeader>();
const FREE_HEADER_SIZE: usize = align_of::<FreeHeader>();

#[derive(Debug)]
struct FreeHandle<'heap> {
    data: &'heap mut FreeHeader,
    offset: usize,
}

impl<'heap> FreeHandle<'heap> {
    fn range_exclusive(&self) -> (usize, usize) {
        (self.offset, self.offset + self.data.size)
    }

    fn range_inclusive(&self) -> (usize, usize) {
        (self.offset, self.offset + self.data.size - 1)
    }

    fn as_ptr_mut(&mut self) -> *mut FreeHeader {
        self.data
    }
}

pub struct KernelHeap {
    pages: VirtPageRange,
    free_head: Option<NonNull<FreeHeader>>,
}

#[derive(Debug)]
struct AllocResult {
    // Where to place the allocation header.
    header_start: usize,
    // The start of the allocated memory - this is the final value that gets returned from alloc.
    alloc_start: usize,
    // The start of the space taken by this allocation - including header and padding.
    used_start: usize,
    // The end of the space taken by this allocation - including header and padding.
    used_end: usize,
    // The remaining space in the free header. May be None if the remaining space is too small for a header.
    remaining: Option<(usize, usize)>,
}

impl KernelHeap {
    // Safety: ptr must point to a valid instance of FreeHeader inside our heap
    // memory area.
    unsafe fn get_free_header_mut<'heap>(
        &'heap mut self,
        ptr: *mut FreeHeader,
    ) -> FreeHandle<'heap> {
        let offset = ptr as usize;

        assert!((offset as u64) >= self.pages.first().base_u64());
        assert!((offset as u64) < self.pages.end().base_u64());

        let data = &mut (*ptr);
        FreeHandle { data, offset }
    }

    // Safety: the range [dst, dst + size] must not be used or referenced.
    // offset must be properly aligned for FreeHeader.
    unsafe fn write_free_header<'heap>(
        &'heap mut self,
        header: FreeHeader,
        offset: usize,
    ) -> FreeHandle<'heap> {
        // Some sanity checks...
        assert!(is_aligned(offset as u64, align_of::<FreeHeader>() as u64));
        assert!(offset as u64 >= self.pages.first().base_u64());
        assert!((offset + header.size) as u64 <= self.pages.end().base_u64());

        let dst = offset as *mut FreeHeader;

        ptr::write(dst, header);

        FreeHandle {
            data: &mut *dst,
            offset,
        }
    }

    // Safety: result must come from try_alloc_from_free_block.
    unsafe fn write_alloc_header(&mut self, result: &AllocResult) {
        let header = AllocHeader {
            start: result.used_start,
            end: result.used_end,
        };

        let ptr = result.header_start as *mut AllocHeader;

        unsafe {
            ptr::write(ptr, header);
        };
    }

    fn try_alloc_from_free_block(
        handle: &FreeHandle,
        size: usize,
        align: usize,
    ) -> Option<AllocResult> {
        let (free_start, free_end) = handle.range_exclusive();

        // Our usable memory starts after allocating space for our allocation header.
        // Align up to ensure we have a start address that's compatible with the requested alignment.
        let alloc_start = align_up((free_start + ALLOC_HEADER_SIZE) as u64, align as u64) as usize;
        // Usable memory is the start + size. Align up to ensure we always start our next free header (or alloc header) at the correct alignment.
        let alloc_end = align_up((alloc_start + size) as u64, FREE_HEADER_ALIGN as u64) as usize;

        let header_start = alloc_start - size_of::<AllocHeader>();
        // Double check that our header start is correctly aligned.
        assert!(is_aligned(
            header_start as u64,
            align_of::<AllocHeader>() as u64
        ));

        let remaining_start = alloc_end;
        let remaining_end = free_end;
        if remaining_end < remaining_start {
            // Allocation's too big. This won't work.
            return None;
        };

        let remaining = if remaining_end - remaining_start < FREE_HEADER_SIZE {
            // If there's not enough space for another free header in this block, then the remaining space is None.
            None
        } else {
            // Otherwise, return the remaining range.
            Some((remaining_start, remaining_end))
        };

        // The start of the used space is just the start of this block.
        let used_start = free_start;
        // If we have some space for another header, the used_end is the end of the allocation space.
        // Otherwise, the used_end is the end of this block.
        let used_end = match remaining {
            Some(_) => alloc_end,
            None => free_end,
        };

        Some(AllocResult {
            header_start,
            used_start,
            alloc_start,
            used_end,
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
        let initial_header = FreeHeader {
            prev: None,
            next: None,
            size,
        };

        let start_offset = pages.first().base_u64() as usize;
        let mut handle = heap.write_free_header(initial_header, start_offset);
        heap.free_head =
            Some(NonNull::new(handle.as_ptr_mut()).expect("heap initial head should be non-null"));

        heap
    }

    pub fn alloc(&mut self, layout: Layout) -> *mut u8 {
        // TODO: actually iterate over the free list.
        let mut head = self.free_head.unwrap();

        // We assume the head pointer is always valid.
        let head = unsafe { self.get_free_header_mut(head.as_mut()) };
        let result = match Self::try_alloc_from_free_block(&head, layout.size(), layout.align()) {
            Some(r) => r,
            // No memory left.
            None => return null_mut(),
        };

        // Comes direct from try_alloc_from_free_block.
        unsafe { self.write_alloc_header(&result) };

        if let Some((remaining_start, remaining_end)) = result.remaining {
            let header = FreeHeader {
                prev: None,
                next: None,
                size: remaining_end - remaining_start,
            };
            // remaining_start came from inside a free block, and is guaranteed to have enough space
            // for a header by try_alloc_from_free_block.
            let mut handle = unsafe { self.write_free_header(header, remaining_start) };
            self.free_head = NonNull::new(handle.as_ptr_mut());
        } else {
            self.free_head = None;
        }

        result.alloc_start as *mut u8
    }

    pub fn free(&mut self, ptr: *mut u8, layout: Layout) {}
}
