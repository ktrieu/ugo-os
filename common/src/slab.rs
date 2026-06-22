use core::{mem::MaybeUninit, num::NonZero};

use crate::addr::{Address, Page, VirtAddr, VirtPageRange};

pub struct SlabAllocator {
    pages: VirtPageRange,
    // Size of one slot in bytes.
    size: u32,
    // Offset of first free slot in this allocator.
    head: Option<SlotIndex>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
// We use NonZero so we can get "free" Options of this type. It does mean that
// 1 is the first slot.
struct SlotIndex(NonZero<u32>);

impl SlotIndex {
    fn new(idx: u32) -> Self {
        Self(NonZero::new(idx + 1).expect("idx must be > 0"))
    }

    fn raw_idx(&self) -> u32 {
        self.0.get() - 1
    }
}

#[derive(Clone, Copy)]
struct SlotHeader {
    // The next free slot in the allocator.
    next: Option<SlotIndex>,
}

impl SlabAllocator {
    // `pages` must be clear for writes. `size` must be a power of 2.
    pub unsafe fn new(pages: VirtPageRange, size: u32) -> Self {
        assert!(size.is_power_of_two());

        let mut allocator = Self {
            pages,
            size,
            head: None,
        };

        // Initialize each slot to point to the next.
        for raw_idx in 0..allocator.num_slots() {
            let next = if raw_idx + 1 == allocator.num_slots() {
                None
            } else {
                Some(allocator.slot_index(raw_idx + 1))
            };

            let header = allocator.header_ptr_mut(allocator.slot_index(raw_idx));
            // All slots are free! This is fine.
            unsafe {
                (*header).write(SlotHeader { next });
            }
        }

        // And then hook up the head offset to the first slot.
        allocator.head = Some(allocator.slot_index(0));

        allocator
    }

    fn num_slots(&self) -> u32 {
        (self.pages.len_bytes() as u32 / self.size) as u32
    }

    fn byte_offset(&self, idx: SlotIndex) -> usize {
        (self.size * idx.raw_idx()) as usize
    }

    fn raw_idx(&self, ptr: *mut u8) -> u32 {
        assert!(self.owns_ptr(ptr));

        let diff = ptr as u64 - self.pages.first().base_u64();
        let diff: u32 = diff.try_into().unwrap();

        let slot_count = diff / self.size;
        // This had better be an exact multiple...
        assert!(diff % self.size == 0);

        slot_count
    }

    fn slot_index(&self, idx: u32) -> SlotIndex {
        assert!(idx < self.num_slots());

        SlotIndex::new(idx)
    }

    fn header_ptr_mut(&mut self, idx: SlotIndex) -> *mut MaybeUninit<SlotHeader> {
        let base = self.pages.first().base_addr().as_u8_ptr_mut();

        // SlabOffsets always point to a location inside this page range.
        unsafe { base.byte_add(self.byte_offset(idx)) as *mut MaybeUninit<SlotHeader> }
    }

    pub fn alloc(&mut self) -> Option<*mut u8> {
        let to_alloc = self.header_ptr_mut(self.head?);

        // Wire up our `head` ptr to the next free cell after to_alloc.
        // Safety: If self.head is not None it should always point to a free slot.
        unsafe {
            self.head = (*to_alloc).assume_init().next;
        };

        // Cast our ptr to a generic one so we can allocate it.
        Some(to_alloc as *mut u8)
    }

    // Is this a pointer that came from this slab allocator?
    pub fn owns_ptr(&self, ptr: *mut u8) -> bool {
        let raw = ptr as u64;

        self.pages.contains_addr(VirtAddr::new(raw))
    }

    pub fn free(&mut self, ptr: *mut u8) {
        let raw_idx = self.raw_idx(ptr);
        let slot_index = self.slot_index(raw_idx);

        let ptr = self.header_ptr_mut(slot_index);
        // This memory is free to write since we just got handed it back.
        unsafe { (*ptr).write(SlotHeader { next: self.head }) };

        self.head = Some(slot_index);
    }
}

#[cfg(test)]
mod test {
    use std::collections::HashSet;

    use crate::{
        addr::{Page, VirtPage},
        slab::SlabAllocator,
    };

    #[repr(align(4096))]
    #[derive(Clone, Copy)]
    struct FakePage {
        _data: [u8; 4096],
    }

    impl Default for FakePage {
        fn default() -> Self {
            Self { _data: [0; 4096] }
        }
    }

    struct TestAllocator<const N: usize> {
        _storage: Box<[FakePage; N]>,
        allocator: SlabAllocator,
    }

    impl<const N: usize> TestAllocator<N> {
        fn new(size: u32) -> Self {
            let storage = Box::new([FakePage::default(); N]);
            let start = VirtPage::from_base_u64(storage.as_ptr() as u64);
            let range = VirtPage::range_length(start, 2);

            let allocator = unsafe { SlabAllocator::new(range, size) };

            Self {
                _storage: storage,
                allocator,
            }
        }
    }

    #[test]
    fn test_init() {
        // Initialization should not fail.
        let _allocator = TestAllocator::<2>::new(2048);
    }

    #[test]
    fn test_alloc() {
        let TestAllocator { mut allocator, .. } = TestAllocator::<2>::new(2048);
        // With a 2048 slot size in two pages, allocation should succeed 4 times and then return None.
        let a1 = allocator.alloc().unwrap();
        let a2 = allocator.alloc().unwrap();
        let a3 = allocator.alloc().unwrap();
        let a4 = allocator.alloc().unwrap();

        // These should all point to different pointers.
        let hs = HashSet::from([a1, a2, a3, a4]);
        assert!(hs.len() == 4);

        // And should be inside the allocators range.
        assert!(allocator.owns_ptr(a1));
        assert!(allocator.owns_ptr(a2));
        assert!(allocator.owns_ptr(a3));
        assert!(allocator.owns_ptr(a4));

        // We're all full! Next alloc should return None.
        assert!(allocator.alloc().is_none());
    }

    #[test]
    fn test_free() {
        let TestAllocator { mut allocator, .. } = TestAllocator::<2>::new(2048);

        // Alloc 4 times.
        allocator.alloc().unwrap();
        allocator.alloc().unwrap();
        allocator.alloc().unwrap();
        let last = allocator.alloc().unwrap();

        // Hand back our last pointer which should succeed.
        allocator.free(last);

        // Fetch a new pointer - this should be the same one.
        let next = allocator.alloc().unwrap();
        assert!(last == next);
    }

    #[test]
    fn test_free_middle() {
        let TestAllocator { mut allocator, .. } = TestAllocator::<2>::new(2048);

        // Alloc 4 times.
        allocator.alloc().unwrap();
        allocator.alloc().unwrap();
        let middle = allocator.alloc().unwrap();
        allocator.alloc().unwrap();

        // Hand back our last pointer which should succeed.
        allocator.free(middle);

        // Fetch a new pointer - this should be the same one since it's the only one left.
        let next = allocator.alloc().unwrap();
        assert!(middle == next);
    }
}
