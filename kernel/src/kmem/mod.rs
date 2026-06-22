use core::{
    alloc::{GlobalAlloc, Layout},
    ptr::null_mut,
};

use common::{
    addr::{Page, VirtPage, VirtPageRange},
    slab::SlabAllocator,
    BootInfo, KernelAddresses,
};
use conquer_once::spin::OnceCell;

use crate::{
    kmem::{
        page::{KernelPageTables, MappingType},
        phys::PhysFrameAllocator,
    },
    sync::InterruptSafeSpinlock,
};

pub mod page;
pub mod phys;
pub struct GlobalMemoryManager(OnceCell<InterruptSafeSpinlock<KernelMemoryManager>>);

#[global_allocator]
pub static GLOBAL_MEM: GlobalMemoryManager = GlobalMemoryManager(OnceCell::uninit());

unsafe impl GlobalAlloc for GlobalMemoryManager {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        let Some(mut manager) = self.0.get().map(|l| l.lock()) else {
            return null_mut();
        };

        manager.heap_alloc(layout)
    }

    unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
        let Some(mut manager) = self.0.get().map(|l| l.lock()) else {
            return;
        };

        manager.heap_free(ptr, layout);
    }
}

pub struct KernelMemoryManager {
    _phys_allocator: PhysFrameAllocator,
    _page_tables: KernelPageTables<'static>,
    bootstrap_slab: SlabAllocator,
}

impl KernelMemoryManager {
    const KERNEL_BOOTSTRAP_PAGES: u64 = 2;
    const KERNEL_BOOTSTRAP_SLAB_SIZE: u32 = 32;

    fn bootstrap_heap_area(
        addresses: &KernelAddresses,
        allocator: &mut PhysFrameAllocator,
        page_tables: &mut KernelPageTables,
    ) -> VirtPageRange {
        let start = VirtPage::from_containing_addr(addresses.stack_top).next();

        let heap_pages = VirtPage::range_length(start, Self::KERNEL_BOOTSTRAP_PAGES);

        for page in heap_pages.iter() {
            page_tables.alloc_and_map_page(page, MappingType::DataRw, allocator);
        }

        heap_pages
    }

    pub fn new(boot_info: &'static BootInfo) -> Self {
        let mut page_tables = KernelPageTables::new();
        let mut phys_allocator = PhysFrameAllocator::new(boot_info.mem_regions);

        let bootstrap_pages = Self::bootstrap_heap_area(
            &boot_info.kernel_addrs,
            &mut phys_allocator,
            &mut page_tables,
        );

        // Safety: bootstrap_heap_area returns unused memory from beyond the kernel stack and
        // maps it in as RW.
        let bootstrap_slab =
            unsafe { SlabAllocator::new(bootstrap_pages, Self::KERNEL_BOOTSTRAP_SLAB_SIZE) };

        Self {
            _phys_allocator: phys_allocator,
            _page_tables: page_tables,
            bootstrap_slab,
        }
    }

    pub fn heap_alloc(&mut self, layout: Layout) -> *mut u8 {
        // For now we only we support 32 bytes or less.
        assert!(layout.size() < Self::KERNEL_BOOTSTRAP_SLAB_SIZE as usize);
        assert!(layout.align() < Self::KERNEL_BOOTSTRAP_SLAB_SIZE as usize);
        self.bootstrap_slab.alloc().unwrap()
    }

    pub fn heap_free(&mut self, ptr: *mut u8, _: Layout) {
        assert!(self.bootstrap_slab.owns_ptr(ptr));
        self.bootstrap_slab.free(ptr);
    }

    pub fn register_global(self) {
        GLOBAL_MEM.0.init_once(|| InterruptSafeSpinlock::new(self));
    }
}
