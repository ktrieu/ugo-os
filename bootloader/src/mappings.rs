use core::arch::asm;

use common::{KERNEL_START, PHYSMEM_START};
use uefi::table::boot::MemoryMap;

use crate::{
    addr::{PhysAddr, PhysFrame, VirtAddr, VirtPage},
    frame::FrameAllocator,
    page::{IntermediatePageTable, PageMapLevel4},
    page::{PageMapLevel1, PageTable},
};

pub enum MappingType {
    Code,
    ReadData,
    ReadWriteData,
}

fn map_page_entry(frame: PhysFrame, page: VirtPage, table: &mut PageMapLevel1, ty: MappingType) {
    let entry = table.get_entry_mut(page.base_addr());

    entry.set_addr(frame.base_addr());
    entry.set_present(true);

    match ty {
        MappingType::Code => {
            entry.set_no_exec(false);
            entry.set_write(false);
        }
        MappingType::ReadData => {
            entry.set_no_exec(true);
            entry.set_write(false);
        }
        MappingType::ReadWriteData => {
            entry.set_no_exec(true);
            entry.set_write(true);
        }
    }
}

// Ensure any kernel segment we attempt to map is in the higher half.
// This catches improper base address specification for the kernel.
fn is_valid_kernel_addr(addr: u64) -> bool {
    addr >= KERNEL_START
}

type ElfResult<T> = Result<T, &'static str>;

pub struct Mappings<'a> {
    level_4_map: &'a mut PageMapLevel4,
    level_4_phys_addr: PhysAddr,
}

impl<'a> Mappings<'a> {
    pub fn new(allocator: &mut FrameAllocator) -> Self {
        let (level_4_map, level_4_phys_addr) = PageMapLevel4::alloc_new(allocator);
        Mappings {
            level_4_map,
            level_4_phys_addr,
        }
    }

    pub fn map_page(
        &mut self,
        frame: PhysFrame,
        page: VirtPage,
        allocator: &mut FrameAllocator,
        ty: MappingType,
    ) {
        let addr = page.base_addr();

        let level_3_map = self.level_4_map.get_mut_or_insert(addr, allocator);
        let level_2_map = level_3_map.get_mut_or_insert(addr, allocator);
        let level_1_map = level_2_map.get_mut_or_insert(addr, allocator);

        map_page_entry(frame, page, level_1_map, ty);
    }

    pub fn map_physical_memory(&mut self, memory_map: &MemoryMap, allocator: &mut FrameAllocator) {
        let highest_segment = memory_map
            .entries()
            .max_by_key(|descriptor| descriptor.phys_start)
            .expect("Memory map was empty!");

        let end_frame = PhysFrame::from_base_u64(highest_segment.phys_start)
            .add_frames(highest_segment.page_count);

        bootlog!(
            "Mapping all physical memory.\nP: {:#016x} - {:#016x}\nV: {:#016x} - {:#016x}",
            0,
            end_frame.base_addr().as_u64(),
            PHYSMEM_START,
            PHYSMEM_START + end_frame.base_addr().as_u64()
        );

        let mut frame = PhysFrame::from_base_u64(0);
        let mut page = VirtPage::from_base_addr(VirtAddr::new(PHYSMEM_START));

        while frame.base_addr() < end_frame.base_addr() {
            self.map_page(frame, page, allocator, MappingType::ReadWriteData);
            frame = frame.next_frame();
            page = page.next_page();
        }
    }

    // fn_ptr should be derived from a function, but I can't have a "pointer to any function"
    // as a type, AFAIK.
    pub fn identity_map_fn(&mut self, fn_ptr: *const (), allocator: &mut FrameAllocator) {
        let addr = fn_ptr as u64;

        let frame = PhysFrame::from_containing_u64(addr);
        let page = VirtPage::from_containing_u64(addr);

        bootlog!("Identity mapping {:#016x}", frame.base_addr().as_u64());

        self.map_page(frame, page, allocator, MappingType::Code);
        // The function might lie on a page boundary, so map the next one too.
        bootlog!(
            "Identity mapping {:016x}",
            frame.next_frame().base_addr().as_u64()
        );
        self.map_page(
            frame.next_frame(),
            page.next_page(),
            allocator,
            MappingType::Code,
        )
    }

    pub unsafe fn activate(&self) {
        asm!(
            "mov cr3, {addr}",
            addr = in(reg) self.level_4_phys_addr.as_u64()
        );
    }

    pub fn level_4_phys_addr(&self) -> PhysAddr {
        self.level_4_phys_addr
    }
}
