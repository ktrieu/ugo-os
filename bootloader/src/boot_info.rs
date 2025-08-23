use core::{
    alloc::Layout,
    iter::Peekable,
    mem::{align_of, size_of, MaybeUninit},
    ptr, slice,
};

use common::{
    addr::{Address, Page, PageRange, PageRangeIter, PhysAddr, PhysFrame, VirtPage},
    BootInfo, FramebufferFormat, FramebufferInfo, MemRegion, MemRegions, RegionType, BOOTINFO_SIZE,
    BOOTINFO_START, PAGE_SIZE,
};
use uefi::{
    proto::console::gop::PixelFormat,
    table::boot::{MemoryDescriptor, MemoryMap, MemoryMapIter, MemoryType},
};

use crate::{
    frame::FrameAllocator,
    graphics::Framebuffer,
    mappings::{MappingFlags, Mappings},
};

// Bump allocator for allocating inside a frame.
struct BootInfoAllocator {
    current: PhysAddr,
    frame: PhysFrame,
}

impl BootInfoAllocator {
    pub fn new(allocator: &mut FrameAllocator) -> Self {
        let frame = allocator.alloc_frame();
        Self {
            current: frame.base_addr(),
            frame,
        }
    }

    pub fn frame(&self) -> PhysFrame {
        self.frame
    }

    pub fn allocate<T>(&mut self) -> &'static mut MaybeUninit<T> {
        let size = size_of::<T>();
        let align = align_of::<T>();

        let next = self.current.align_up(align as u64);
        self.current = PhysAddr::new(next.as_u64() + size as u64);
        if self.current.as_u64() >= self.frame.next().base_u64() {
            panic!("No space left in BootInfoAllocator!");
        }

        let ptr = next.as_u64() as *mut MaybeUninit<T>;
        unsafe { &mut *ptr }
    }

    pub fn allocate_array<T>(&mut self, len: usize) -> &'static mut [MaybeUninit<T>] {
        let layout = Layout::array::<T>(len).unwrap();

        let next = self.current.align_up(layout.align() as u64);
        self.current = PhysAddr::new(next.as_u64() + layout.size() as u64);
        if self.current.as_u64() >= self.frame.next().base_u64() {
            panic!("No space left in BootInfoAllocator!");
        }

        let ptr = next.as_u64() as *mut MaybeUninit<T>;
        unsafe { slice::from_raw_parts_mut(ptr, len) }
    }
}

// A struct for allocating pages in the BootInfo section.
pub struct BootInfoPageAllocator {
    iter: Peekable<PageRangeIter<VirtPage>>,
}

impl BootInfoPageAllocator {
    pub fn new(range: PageRange<VirtPage>) -> Self {
        Self {
            iter: range.iter().peekable(),
        }
    }

    pub fn alloc(&mut self) -> VirtPage {
        match self.iter.next() {
            Some(page) => page,
            None => panic!("No virtual memory remaining in BootInfo section!"),
        }
    }

    pub fn alloc_pages(&mut self, n: u64) -> PageRange<VirtPage> {
        let first = self
            .iter
            .peek()
            .copied()
            .expect("No virtual memory remaining in BootInfo section!");

        for _ in 0..n {
            self.alloc();
        }

        let range = VirtPage::range_length(first, n);
        // Make sure the range we return matches our inner state.
        assert!(Some(&range.last().next()) == self.iter.peek());
        range
    }
}

fn new_framebuffer_info(framebuffer: &Framebuffer) -> FramebufferInfo {
    let height = framebuffer.height() as usize;
    let width = framebuffer.width() as usize;
    // For now: we use BGR format only when selecting modes.
    let format = match framebuffer.format() {
        PixelFormat::Bgr => FramebufferFormat::Bgr,
        _ => unimplemented!("Unsupported framebuffer format for boot info."),
    };
    FramebufferInfo {
        // We'll deal with address selection later.
        address: ptr::null_mut(),
        format,
        stride: framebuffer.stride() as usize,
        width,
        height,
    }
}

fn new_mem_region(descriptor: &MemoryDescriptor) -> MemRegion {
    let ty = match descriptor.ty {
        MemoryType::CONVENTIONAL => RegionType::Usable,
        // We've allocated frames for the kernel executable image in here, so we can't re-use it.
        MemoryType::LOADER_DATA => RegionType::Allocated,
        // We're not going to need the loader code after we switch to the OS.
        MemoryType::LOADER_CODE => RegionType::Usable,
        // Ditto for boot services memory.
        MemoryType::BOOT_SERVICES_CODE | MemoryType::BOOT_SERVICES_DATA => RegionType::Usable,
        // Everything else is unusable.
        _ => RegionType::Allocated,
    };

    MemRegion {
        start: descriptor.phys_start,
        pages: descriptor.page_count,
        ty,
    }
}

struct MemRegionIter<'map> {
    entries: Peekable<MemoryMapIter<'map>>,
}

impl<'map> MemRegionIter<'map> {
    pub fn new(memory_map: &'map MemoryMap) -> Self {
        Self {
            entries: memory_map.entries().peekable(),
        }
    }
}

impl<'map> Iterator for MemRegionIter<'map> {
    type Item = MemRegion;

    fn next(&mut self) -> Option<Self::Item> {
        // Iterate through the underlying memory descriptors, combining memory descriptors of the same type.
        // Only yield when the type changes.

        let mut region = new_mem_region(self.entries.next()?);

        while let Some(descriptor) = self.entries.peek() {
            let peek_region = new_mem_region(&descriptor);

            let peek_range = peek_region.as_range();
            let region_range = region.as_range();

            // If the types are the same and they're contiguous - merge them together.
            if peek_region.ty == region.ty && region_range.end() == peek_range.first() {
                region.pages += peek_region.pages;
            } else {
                break;
            }

            let _ = self.entries.next();
        }

        Some(region)
    }
}

fn create_mem_regions(
    memory_map: MemoryMap,
    frame_allocator: &mut FrameAllocator,
    boot_info_alloc: &mut BootInfoAllocator,
) -> (&'static mut [MaybeUninit<MemRegion>], usize) {
    let region_iter = MemRegionIter::new(&memory_map);
    // The frame allocator will fall in one of these
    // We'll have to split that region into two extra pieces to account for that.
    let num_regions = region_iter.count() + 2;
    let mem_regions = boot_info_alloc.allocate_array::<MemRegion>(num_regions);

    let region_iter = MemRegionIter::new(&memory_map);

    let alloc_reserved = frame_allocator.reserved_range();

    let mut idx = 0;
    for region in region_iter {
        let region_range = region.as_range();
        if region_range.contains_range(alloc_reserved) {
            // We should always allocate our boot memory from a usable region.
            assert!(region.ty == RegionType::Usable);

            let pre_range =
                PhysFrame::range_exclusive(region_range.first(), alloc_reserved.first());
            let post_range = PhysFrame::range_inclusive(alloc_reserved.end(), region_range.last());

            if pre_range.len() > 0 {
                mem_regions[idx].write(MemRegion::from_range(pre_range, region.ty));
                idx += 1;
            }

            mem_regions[idx].write(MemRegion::from_range(
                frame_allocator.used_range(),
                RegionType::Bootloader,
            ));
            idx += 1;

            if post_range.len() > 0 {
                mem_regions[idx].write(MemRegion::from_range(post_range, region.ty));
                idx += 1;
            }
        } else {
            mem_regions[idx].write(region);
            idx += 1;
        }
    }

    // We may not use all the allocated memory. Return the actual length as well.
    (mem_regions, idx)
}

fn map_framebuffer(
    framebuffer: &Framebuffer,
    frame_allocator: &mut FrameAllocator,
    page_alloc: &mut BootInfoPageAllocator,
    mappings: &mut Mappings,
) -> PageRange<VirtPage> {
    let framebuffer_len = framebuffer.byte_len();

    let start_frame = PhysFrame::from_containing_u64(framebuffer.addr() as u64);
    let end_frame = PhysFrame::from_containing_u64(framebuffer.addr() as u64 + framebuffer_len);

    let frames = PhysFrame::range_inclusive(start_frame, end_frame);
    bootlog!(
        "Framebuffer: {} - {} ({} bytes, {} pages)",
        start_frame,
        end_frame,
        framebuffer_len,
        frames.len()
    );
    let pages = page_alloc.alloc_pages(frames.len());

    bootlog!(
        "Mapping framebuffer: {} - {} ({} pages)",
        pages.first(),
        pages.last(),
        pages.len()
    );
    mappings.map_page_range(frames, pages, frame_allocator, MappingFlags::new_rw_data());

    pages
}

fn fixup_pointer<T>(virtual_offset: u64, pointer: *mut T) -> *mut T {
    (pointer as u64 + virtual_offset) as *mut T
}

pub fn create_boot_info(
    frame_allocator: &mut FrameAllocator,
    mappings: &mut Mappings,
    framebuffer: &Framebuffer,
    memory_map: MemoryMap,
) -> *mut BootInfo {
    let mut boot_info_alloc = BootInfoAllocator::new(frame_allocator);
    let mut boot_page_alloc = BootInfoPageAllocator::new(VirtPage::range_length(
        VirtPage::from_base_u64(BOOTINFO_START),
        BOOTINFO_SIZE / PAGE_SIZE,
    ));

    let boot_info_page = boot_page_alloc.alloc();

    // Map this frame into the boot info section
    mappings.map_page(
        boot_info_alloc.frame(),
        boot_info_page,
        frame_allocator,
        MappingFlags::new_rw_data(),
    );

    // We need this later to generate virtual addresses from the physical addresses we're allocating to.
    let virtual_offset =
        boot_info_page.base_addr().as_u64() - boot_info_alloc.frame.base_addr().as_u64();

    let mut framebuffer_info = new_framebuffer_info(framebuffer);
    let framebuffer_pages =
        map_framebuffer(framebuffer, frame_allocator, &mut boot_page_alloc, mappings);
    let framebuffer_offset = framebuffer_pages.first().base_u64()
        - PhysFrame::from_containing_u64(framebuffer.addr() as u64).base_u64();

    framebuffer_info.address = fixup_pointer(framebuffer_offset, framebuffer.addr());

    let (mem_regions, num_regions) =
        create_mem_regions(memory_map, frame_allocator, &mut boot_info_alloc);

    let boot_info = boot_info_alloc.allocate::<BootInfo>();
    boot_info.write(BootInfo {
        mem_regions: MemRegions {
            ptr: fixup_pointer(virtual_offset, mem_regions.as_mut_ptr()) as *mut MemRegion,
            len: num_regions,
        },
        framebuffer: framebuffer_info,
    });

    // Safety: We just initialized it.
    let boot_info_ptr = unsafe { boot_info.assume_init_mut() as *mut BootInfo };
    fixup_pointer(virtual_offset, boot_info_ptr)
}
