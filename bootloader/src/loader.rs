use core::fmt::Display;

use common::{KERNEL_START, PAGE_SIZE};
use xmas_elf::program::{ProgramHeader, Type as ProgramHeaderType};
use xmas_elf::ElfFile;

use crate::addr::{align_up, PhysFrame, VirtPage};
use crate::frame::FrameAllocator;
use crate::mappings::MappingFlags;
use crate::{
    addr::{PhysAddr, VirtAddr},
    mappings::Mappings,
};

pub struct Loader<'a> {
    kernel_offset: PhysAddr,
    elf_file: ElfFile<'a>,
}

pub enum LoaderError {
    InvalidKernelSegmentAddress(VirtAddr),
    ElfFileError(&'static str),
}

impl Display for LoaderError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            LoaderError::InvalidKernelSegmentAddress(vaddr) => {
                write!(f, "Invalid kernel segment address {}", vaddr)
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
    MappingFlags::new(header.flags().is_execute(), header.flags().is_write())
}

impl<'a> Loader<'a> {
    pub fn new(kernel_data: &'a [u8]) -> LoaderResult<Self> {
        let ptr = &kernel_data[0] as *const u8;
        let kernel_offset = PhysAddr::new(ptr as u64);
        Ok(Loader {
            kernel_offset,
            elf_file: ElfFile::new(kernel_data)?,
        })
    }

    fn map_load_segment(
        &self,
        phdr: &ProgramHeader,
        mappings: &mut Mappings,
        allocator: &mut FrameAllocator,
    ) -> LoaderResult<()> {
        let vaddr = VirtAddr::new(phdr.virtual_addr());
        let num_file_pages = align_up(phdr.file_size(), PAGE_SIZE) / PAGE_SIZE;

        if !is_valid_kernel_addr(vaddr) {
            return Err(LoaderError::InvalidKernelSegmentAddress(vaddr));
        }

        let start_frame =
            PhysFrame::from_containing_u64(self.kernel_offset.as_u64() + phdr.offset());
        let end_frame = start_frame.add_frames(num_file_pages);

        let start_page = VirtPage::from_containing_u64(phdr.virtual_addr());
        let end_page = start_page.add_pages(num_file_pages);

        let frames = start_frame.range_inclusive(end_frame);
        let pages = start_page.range_inclusive(end_page);

        for (frame, page) in frames.zip(pages) {
            mappings.map_page(frame, page, allocator, phdr_flags_to_mappings_flags(phdr));
        }

        Ok(())
    }

    // Loads the kernel and returns the virtual address of the entry point.
    pub fn load_kernel(
        &self,
        mappings: &mut Mappings,
        allocator: &mut FrameAllocator,
    ) -> LoaderResult<VirtAddr> {
        for phdr in self.elf_file.program_iter() {
            if matches!(phdr.get_type()?, ProgramHeaderType::Load) {
                self.map_load_segment(&phdr, mappings, allocator)?;
            }
        }

        // We'll deal with this later.
        Ok(VirtAddr::new(KERNEL_START))
    }
}
