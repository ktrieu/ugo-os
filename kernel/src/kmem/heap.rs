use core::ptr::{self, NonNull};

use common::addr::{is_aligned, Address, Page, VirtAddr, VirtPageRange};

#[derive(Debug)]
struct FreeSegment {
    prev: Option<NonNull<FreeSegment>>,
    next: Option<NonNull<FreeSegment>>,
    // Size is inclusive of this header, since once we allocate from this the header will
    // be overwritten.
    size: u64,
}

fn vaddr_range_inclusive(start: *const FreeSegment, size: u64) -> (VirtAddr, VirtAddr) {
    let start = start as u64;
    let end = start + size - 1;
    kprintln!("{} {}", VirtAddr::new(start), VirtAddr::new(end));

    (VirtAddr::new(start), VirtAddr::new(end))
}

pub struct KernelHeap {
    pages: VirtPageRange,
    free_head: Option<NonNull<FreeSegment>>,
}

impl KernelHeap {
    // Safety: the range [dst + size] must not be used or referenced.
    unsafe fn write_free_segment(&mut self, segment: FreeSegment, dst: *mut FreeSegment) {
        // Some sanity checks...
        assert!(is_aligned(dst as u64, align_of::<FreeSegment>() as u64));

        let (start, end) = vaddr_range_inclusive(dst, segment.size);
        assert!(self.pages.contains_addr(start));
        assert!(self.pages.contains_addr(end));

        kprintln!("Writing {:?} to {}", segment, start);
        ptr::write(dst, segment);
    }

    // Safety: pages must refer to memory that is not being used or referenced.
    pub unsafe fn new(pages: VirtPageRange) -> Self {
        let mut heap = Self {
            pages,
            free_head: None,
        };

        let size = pages.len_bytes();
        let initial_segment = FreeSegment {
            prev: None,
            next: None,
            size,
        };

        let head = pages.first().base_addr().as_u8_ptr_mut() as *mut FreeSegment;
        heap.write_free_segment(initial_segment, head);
        heap.free_head = Some(NonNull::new(head).expect("heap initial head should be non-null"));

        heap
    }
}
