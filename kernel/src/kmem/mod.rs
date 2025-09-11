use core::{
    alloc::{GlobalAlloc, Layout},
    ptr::null_mut,
};

use common::BootInfo;
use conquer_once::spin::OnceCell;

use crate::{
    kmem::{heap::KernelHeap, page::KernelPageTables, phys::PhysFrameAllocator},
    sync::InterruptSafeSpinlock,
};

pub mod heap;
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
    heap: KernelHeap,
}

impl KernelMemoryManager {
    pub fn new(boot_info: &'static BootInfo) -> Self {
        let mut page_tables = KernelPageTables::new();
        let mut phys_allocator = PhysFrameAllocator::new(boot_info.mem_regions);

        let heap = KernelHeap::new(
            boot_info.kernel_addrs,
            &mut phys_allocator,
            &mut page_tables,
        );

        Self {
            _phys_allocator: phys_allocator,
            _page_tables: page_tables,
            heap,
        }
    }

    pub fn heap_alloc(&mut self, layout: Layout) -> *mut u8 {
        self.heap.alloc(layout)
    }

    pub fn heap_free(&mut self, ptr: *mut u8, layout: Layout) {
        self.heap.free(ptr, layout);
    }

    pub fn register_global(self) {
        GLOBAL_MEM.0.init_once(|| InterruptSafeSpinlock::new(self));
    }
}
