use common::{
    addr::{Page, VirtPage, VirtPageRange},
    KernelAddresses,
};

use crate::kmem::{
    page::{KernelPageTables, MappingType},
    phys::PhysFrameAllocator,
};

pub struct KernelHeap {
    pages: VirtPageRange,
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

        Self { pages: heap_pages }
    }
}
