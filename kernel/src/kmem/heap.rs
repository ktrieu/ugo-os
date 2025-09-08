use core::{
    alloc::{GlobalAlloc, Layout},
    ptr::null_mut,
};

use common::{
    addr::{Address, Page, VirtAddr, VirtPage, VirtPageRange},
    KernelAddresses,
};
use conquer_once::spin::OnceCell;

use crate::{
    kmem::{
        page::{KernelPageTables, MappingType},
        phys::PhysFrameAllocator,
    },
    sync::InterruptSafeSpinlock,
};

pub struct KernelHeap {
    pages: VirtPageRange,
    top: VirtAddr,
}

impl KernelHeap {
    const KERNEL_HEAP_PAGES: u64 = 10;

    pub fn new(
        addresses: KernelAddresses,
        phys_allocator: &mut PhysFrameAllocator,
        page_tables: &mut KernelPageTables,
    ) -> Self {
        let start = VirtPage::from_containing_addr(addresses.stack_top).next();

        let heap_pages = VirtPage::range_length(start, Self::KERNEL_HEAP_PAGES);

        for page in heap_pages.iter() {
            page_tables.alloc_and_map_page(page, MappingType::DataRw, phys_allocator);
        }

        kprintln!("Reserved pages for kernel heap: {}", heap_pages);

        Self {
            pages: heap_pages,
            top: heap_pages.first().base_addr(),
        }
    }

    pub fn register_global_alloc(self) {
        GLOBAL_HEAP_ALLOC
            .0
            .init_once(|| InterruptSafeSpinlock::new(self))
    }

    fn alloc(&mut self, layout: Layout) -> *mut u8 {
        let aligned = self.top.align_up(layout.align() as u64);

        let alloc_end = VirtAddr::new(aligned.as_u64() + layout.size() as u64);

        if alloc_end > self.pages.end().base_addr() {
            // Uh oh. This allocation would run past our allocated memory. Return null and bail out.
            return null_mut();
        }

        self.top = alloc_end;

        aligned.as_u8_ptr_mut()
    }

    fn free(&mut self, _ptr: *mut u8, _layout: Layout) {
        // We're just bump allocating for now. Free some memory later.
    }
}

pub struct GlobalHeapAllocator(OnceCell<InterruptSafeSpinlock<KernelHeap>>);

#[global_allocator]
static GLOBAL_HEAP_ALLOC: GlobalHeapAllocator = GlobalHeapAllocator(OnceCell::uninit());

unsafe impl GlobalAlloc for GlobalHeapAllocator {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        let Some(mut heap) = self.0.get().map(|l| l.lock()) else {
            return null_mut();
        };

        heap.alloc(layout)
    }

    unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
        let Some(mut heap) = self.0.get().map(|l| l.lock()) else {
            return;
        };

        heap.free(ptr, layout);
    }
}
