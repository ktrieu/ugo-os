use core::{alloc::Layout, ptr::null_mut};

use common::{
    addr::{Address, Page, VirtAddr, VirtPage, VirtPageRange},
    KernelAddresses,
};

use crate::kmem::{
    page::{KernelPageTables, MappingType},
    phys::PhysFrameAllocator,
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

        Self {
            pages: heap_pages,
            top: heap_pages.first().base_addr(),
        }
    }

    pub fn alloc(&mut self, layout: Layout) -> *mut u8 {
        let aligned = self.top.align_up(layout.align() as u64);

        let alloc_end = VirtAddr::new(aligned.as_u64() + layout.size() as u64);

        if alloc_end > self.pages.end().base_addr() {
            // Uh oh. This allocation would run past our allocated memory. Return null and bail out.
            return null_mut();
        }

        self.top = alloc_end;

        aligned.as_u8_ptr_mut()
    }

    pub fn free(&mut self, _ptr: *mut u8, _layout: Layout) {
        // We're just bump allocating for now. Free some memory later.
    }
}
