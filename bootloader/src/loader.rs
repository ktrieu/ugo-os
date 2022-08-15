use uefi::table::boot::MemoryDescriptor;
use x86_64::{
    structures::paging::{
        mapper::MapToError, Mapper, OffsetPageTable, Page, PageTableFlags, PhysFrame, Size4KiB,
    },
    PhysAddr, VirtAddr,
};
use xmas_elf::{
    program::{ProgramHeader, ProgramIter, Type},
    ElfFile,
};

use crate::mem::{
    frame::FrameAllocator,
    valloc::{VirtualAllocError, VirtualAllocator},
};

#[derive(Debug)]
pub enum KernelLoadError {
    ElfFileError(&'static str),
    MappingError(MapToError<Size4KiB>),
    VirtualAllocatorError(VirtualAllocError),
}

fn load_segment<'a, I: ExactSizeIterator<Item = &'a MemoryDescriptor> + Clone>(
    segment: &ProgramHeader,
    phys_offset: u64,
    virtual_offset: u64,
    frame: &mut FrameAllocator<'a, I>,
    virt: &mut VirtualAllocator,
    page_table: &mut OffsetPageTable<'static>,
) -> Result<(), KernelLoadError> {
    if segment.file_size() < segment.mem_size() {
        todo!("Add support for segment zeroing!")
    }

    let phys_start = PhysAddr::new(phys_offset + segment.offset());
    let start_frame = PhysFrame::<Size4KiB>::containing_address(phys_start);

    let virt_start = VirtAddr::new(virtual_offset + segment.virtual_addr());
    let start_page = Page::<Size4KiB>::containing_address(virt_start);
    let virt_end = virt_start + segment.mem_size();
    let end_page = Page::<Size4KiB>::containing_address(virt_end);

    let num_pages = end_page - start_page + 1;
    virt.allocate(num_pages)
        .map_err(|e| KernelLoadError::VirtualAllocatorError(e))?;

    let mut flags = PageTableFlags::PRESENT;
    if !segment.flags().is_execute() {
        flags |= PageTableFlags::NO_EXECUTE;
    }

    if segment.flags().is_write() {
        flags |= PageTableFlags::WRITABLE;
    }

    for page_idx in 0..num_pages {
        unsafe {
            let flusher = page_table
                .map_to(start_page + page_idx, start_frame + page_idx, flags, frame)
                .map_err(|e| KernelLoadError::MappingError(e))?;
            flusher.ignore();
        }
    }

    Ok(())
}

fn get_virtual_offset(segments: ProgramIter, virt: &VirtualAllocator) -> u64 {
    let lowest_vaddr = segments
        .min_by_key(|s| s.virtual_addr())
        .expect("No ELF file segments.")
        .virtual_addr();
    virt.next_addr() - lowest_vaddr
}

pub fn load_kernel<'a, I: ExactSizeIterator<Item = &'a MemoryDescriptor> + Clone>(
    kernel_data: &[u8],
    frame: &mut FrameAllocator<'a, I>,
    virt: &mut VirtualAllocator,
    page_table: &mut OffsetPageTable<'static>,
) -> Result<(), KernelLoadError> {
    let elf_file = ElfFile::new(kernel_data).map_err(|e| KernelLoadError::ElfFileError(e))?;

    let phys_offset = kernel_data.as_ptr() as u64;
    let virtual_offset = get_virtual_offset(elf_file.program_iter(), virt);

    for segment in elf_file.program_iter() {
        let segment_type = segment
            .get_type()
            .map_err(|e| KernelLoadError::ElfFileError(e))?;
        match segment_type {
            Type::Load => {
                load_segment(
                    &segment,
                    phys_offset,
                    virtual_offset,
                    frame,
                    virt,
                    page_table,
                )?;
            }
            Type::Dynamic => {
                todo!("We should really add support for relocation!");
            }
            _ => {}
        }
    }

    Ok(())
}
