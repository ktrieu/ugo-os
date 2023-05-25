use core::cmp::max;
use core::fmt::Display;
use core::ptr::{copy_nonoverlapping, write_bytes};

use common::{KERNEL_START, PAGE_SIZE};
use xmas_elf::program::{ProgramHeader, Type as ProgramHeaderType};
use xmas_elf::ElfFile;

use crate::addr::{is_aligned, Address, Page, PhysFrame, VirtPage};
use crate::frame::FrameAllocator;
use crate::mappings::MappingFlags;
use crate::{
    addr::{PhysAddr, VirtAddr},
    mappings::Mappings,
};

pub struct KernelAddresses {
    pub kernel_end: VirtAddr,
    pub kernel_entry: VirtAddr,
    pub stack_top: VirtAddr,
    pub stack_pages: u64,
}

pub struct Loader<'a> {
    kernel_phys_offset: PhysAddr,
    elf_file: ElfFile<'a>,
}

pub enum LoaderError {
    InvalidKernelSegmentAddress(VirtAddr),
    ImproperAlignment(u64),
    ElfFileError(&'static str),
}

impl Display for LoaderError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            LoaderError::InvalidKernelSegmentAddress(vaddr) => {
                write!(f, "Invalid kernel segment address {}", vaddr)
            }
            LoaderError::ImproperAlignment(align) => {
                write!(f, "Improperly aligned segment (aligned {})", align)
            }
            LoaderError::ElfFileError(elf_error) => write!(f, "ELF file read error: {}", elf_error),
        }
    }
}

// This isn't strictly a valid conversion, since this only applies for &'static str's we get
// from the xmas_elf library, but it's convenient, and we'll really only use this here.
impl From<&'static str> for LoaderError {
    fn from(error_str: &'static str) -> Self {
        LoaderError::ElfFileError(error_str)
    }
}

pub type LoaderResult<T> = Result<T, LoaderError>;

// Check if this is after the start of kernel code in the virtual memory map.
// This ensures we link the kernel at the right base address.
fn is_valid_kernel_addr(addr: VirtAddr) -> bool {
    addr.as_u64() >= KERNEL_START
}

fn phdr_flags_to_mappings_flags(header: &ProgramHeader) -> MappingFlags {
    MappingFlags::new(header.flags().is_execute(), header.flags().is_write(), true)
}

// Doesn't really matter what this is, probably.
const KERNEL_STACK_PAGES: u64 = 3;

impl<'a> Loader<'a> {
    pub fn new(kernel_data: &'a [u8]) -> LoaderResult<Self> {
        let ptr = &kernel_data[0] as *const u8;
        let kernel_offset = PhysAddr::new(ptr as u64);
        Ok(Loader {
            kernel_phys_offset: kernel_offset,
            elf_file: ElfFile::new(kernel_data)?,
        })
    }

    fn map_zeroed_memory(
        &self,
        phdr: &ProgramHeader,
        mappings: &mut Mappings,
        allocator: &mut FrameAllocator,
    ) -> LoaderResult<()> {
        let phdr_phys_start = PhysAddr::new(self.kernel_phys_offset.as_u64() + phdr.offset());
        let virtual_offset = phdr.virtual_addr() - phdr_phys_start.as_u64();
        let zero_mem_start = PhysAddr::new(phdr_phys_start.as_u64() + phdr.file_size());

        if !is_aligned(zero_mem_start.as_u64(), PAGE_SIZE) {
            // If the zeroed section isn't aligned, we have to copy the last non-zero frame to a new frame
            // then zero the required memory. This is because the part of the frame in the file that should be zeroed
            // almost certainly contains other data.
            let src_frame = PhysFrame::from_containing_addr(zero_mem_start);
            let src_offset_in_frame = phdr_phys_start.as_u64() - src_frame.base_u64();

            let dst_frame = allocator.alloc_frame();
            let copy_dst = PhysAddr::new(dst_frame.base_u64() + src_offset_in_frame);

            // Zero our frame.
            // Safety: Since dst_frame is a fresh page, it's aligned and clear to write a page of zeroes to.
            unsafe {
                write_bytes(dst_frame.as_u8_ptr_mut(), 0, PAGE_SIZE as usize);
            }

            // We need to copy from the src_offset to the end of the non-zeroed data.
            let bytes_to_copy = zero_mem_start.as_u64() - phdr_phys_start.as_u64();

            // Safety: src_frame and dst_frame are guaranteed to not overlap, since dst_frame is freshly allocated
            // from free memory via FrameAllocator.
            unsafe {
                copy_nonoverlapping(
                    phdr_phys_start.as_u8_ptr(),
                    copy_dst.as_u8_ptr_mut(),
                    bytes_to_copy as usize,
                )
            }

            let virtual_address = VirtAddr::new(zero_mem_start.as_u64() + virtual_offset);

            mappings.map_page(
                dst_frame,
                VirtPage::from_containing_addr(virtual_address),
                allocator,
                phdr_flags_to_mappings_flags(phdr),
            );
        };

        // Since we've handled the unaligned case above, we can now align the address up and deal with the remaining pages.
        let zero_mem_start = zero_mem_start.align_up(PAGE_SIZE);
        if phdr_phys_start.as_u64() + phdr.mem_size() > zero_mem_start.as_u64() {
            // We just aligned this, so this is a base address.
            let start_page = VirtPage::from_base_u64(zero_mem_start.as_u64() + virtual_offset);
            let end_page = VirtPage::from_containing_u64(phdr.virtual_addr() + phdr.mem_size());
            let pages = VirtPage::range_inclusive(start_page, end_page);

            let frames =
                mappings.alloc_and_map_range(pages, allocator, phdr_flags_to_mappings_flags(phdr));

            for frame in frames.iter() {
                // Zero the frames we allocated.
                // Safety: Since dst_frame is a fresh page, it's aligned and clear to write a page of zeroes to.
                unsafe {
                    write_bytes(frame.as_u8_ptr_mut(), 0, PAGE_SIZE as usize);
                }
            }
        }

        Ok(())
    }

    fn map_load_segment(
        &self,
        phdr: &ProgramHeader,
        mappings: &mut Mappings,
        allocator: &mut FrameAllocator,
    ) -> LoaderResult<()> {
        let vaddr = VirtAddr::new(phdr.virtual_addr());
        let pages = VirtPage::range_inclusive(
            VirtPage::from_containing_addr(vaddr),
            VirtPage::from_containing_u64(phdr.virtual_addr() + phdr.file_size()),
        );

        if !is_valid_kernel_addr(vaddr) {
            return Err(LoaderError::InvalidKernelSegmentAddress(vaddr));
        }

        if phdr.align() != PAGE_SIZE {
            return Err(LoaderError::ImproperAlignment(phdr.align()));
        }

        let start_frame =
            PhysFrame::from_containing_u64(self.kernel_phys_offset.as_u64() + phdr.offset());

        let frames = PhysFrame::range_length(start_frame, pages.len());

        mappings.map_page_range(frames, pages, allocator, phdr_flags_to_mappings_flags(phdr));

        if phdr.mem_size() > phdr.file_size() {
            self.map_zeroed_memory(phdr, mappings, allocator)?;
        }

        Ok(())
    }

    fn create_kernel_stack(
        &mut self,
        kernel_end: VirtAddr,
        mappings: &mut Mappings,
        allocator: &mut FrameAllocator,
    ) -> VirtAddr {
        // Just grab the next frame after the kernel to start the stack.

        // First a guard page. Just map the zero frame here.
        let guard_page = VirtPage::from_containing_addr(kernel_end).next();
        mappings.map_page(
            PhysFrame::from_base_u64(0),
            guard_page,
            allocator,
            MappingFlags::new_guard(),
        );

        bootlog!("Allocating guard page at {}", guard_page);

        // Next, allocate the actual stack.
        let stack_start = guard_page.next();
        let stack_pages = VirtPage::range_length(stack_start, KERNEL_STACK_PAGES);
        bootlog!("Stack start at {}", stack_pages.first());
        bootlog!("Stack end at {}", stack_pages.last());
        mappings.alloc_and_map_range(stack_pages, allocator, MappingFlags::new_rw_data());

        // The stack starts in the top of the page *before* stack_end, since it's an exclusive range.
        // So, we need to bump the stack top address down. SystemV ABI says the pointer has to be aligned
        // to a 16 byte boundary, so subtract that amount.
        VirtAddr::new(stack_pages.last().base_u64() - 16)
    }

    // Loads the kernel and returns the virtual address of the entry point.
    pub fn load_kernel(
        &mut self,
        mappings: &mut Mappings,
        allocator: &mut FrameAllocator,
    ) -> LoaderResult<KernelAddresses> {
        let mut kernel_end = VirtAddr::new(0);

        for phdr in self.elf_file.program_iter() {
            if matches!(phdr.get_type()?, ProgramHeaderType::Load) {
                self.map_load_segment(&phdr, mappings, allocator)?;
                // Update the end of the kernel.
                kernel_end = VirtAddr::new(max(
                    kernel_end.as_u64(),
                    phdr.virtual_addr() + phdr.mem_size(),
                ));
            }
        }

        bootlog!("Kernel end at {}", kernel_end);

        let stack_top = self.create_kernel_stack(kernel_end, mappings, allocator);
        let kernel_entry = VirtAddr::new(self.elf_file.header.pt2.entry_point());

        let addresses = KernelAddresses {
            kernel_end,
            kernel_entry,
            stack_top,
            stack_pages: KERNEL_STACK_PAGES,
        };

        Ok(addresses)
    }
}
