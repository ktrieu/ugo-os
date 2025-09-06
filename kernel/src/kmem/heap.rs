use common::{
    addr::{Page, VirtPage, VirtPageRange},
    KernelAddresses,
};

pub struct KernelHeap {
    pages: VirtPageRange,
}

impl KernelHeap {
    const KERNEL_HEAP_PAGES: u64 = 10;

    pub fn new(addresses: KernelAddresses) -> Self {
        let start = VirtPage::from_containing_addr(addresses.stack_top).next();

        let heap_pages = VirtPage::range_length(start, Self::KERNEL_HEAP_PAGES);

        kprintln!("Reserved pages for kernel heap: {}", heap_pages);

        Self { pages: heap_pages }
    }
}
