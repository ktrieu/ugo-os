use core::{
    alloc::Layout,
    mem::{align_of, size_of, MaybeUninit},
    ptr, slice,
};

use common::{
    BootInfo, FramebufferFormat, FramebufferInfo, MemRegion, MemRegions, RegionType, BOOTINFO_START,
};
use uefi::{
    proto::console::gop::PixelFormat,
    table::boot::{MemoryDescriptor, MemoryMap, MemoryType},
};

use crate::{
    addr::{Address, Page, PhysAddr, PhysFrame, VirtPage},
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
    let page = VirtPage::from_base_u64(BOOTINFO_START);

    // Map this frame into the boot info section
    mappings.map_page(
        boot_info_alloc.frame(),
        page,
        frame_allocator,
        MappingFlags::new_rw_data(),
    );

    // We need this later to generate virtual addresses from the physical addresses we're allocating to.
    let virtual_offset = page.base_addr().as_u64() - boot_info_alloc.frame.base_addr().as_u64();

    let framebuffer_info = new_framebuffer_info(framebuffer);

    // Remember, we've split one memory map section into two for the frame allocator.
    let num_mem_regions = memory_map.entries().len() + 1;
    let mem_regions = boot_info_alloc.allocate_array::<MemRegion>(num_mem_regions);

    let mut idx = 0;
    for entry in memory_map.entries() {
        if entry.phys_start == frame_allocator.alloc_start().as_u64() {
            // We need to split this section in half, since we've used some of it to allocate our own memory.
            let frame_alloc_region = MemRegion {
                start: frame_allocator.alloc_start().as_u64(),
                pages: frame_allocator.frames_allocated(),
                ty: RegionType::Bootloader,
            };
            mem_regions[idx].write(frame_alloc_region);
            idx += 1;

            // And the remaining of this section.
            let remaining_region = MemRegion {
                start: frame_allocator.next_frame().base_addr().as_u64(),
                pages: entry.page_count - frame_allocator.frames_allocated(),
                ty: RegionType::Usable,
            };
            mem_regions[idx].write(remaining_region);
            idx += 1
        } else {
            mem_regions[idx].write(new_mem_region(entry));
            idx += 1;
        }
    }

    let boot_info = boot_info_alloc.allocate::<BootInfo>();
    boot_info.write(BootInfo {
        mem_regions: MemRegions {
            ptr: fixup_pointer(virtual_offset, mem_regions.as_mut_ptr()) as *mut MemRegion,
            len: num_mem_regions,
        },
        framebuffer: framebuffer_info,
    });

    // Safety: We just initialized it.
    let boot_info_ptr = unsafe { boot_info.assume_init_mut() as *mut BootInfo };
    fixup_pointer(virtual_offset, boot_info_ptr)
}
