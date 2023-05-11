use core::fmt::Display;

use common::KERNEL_START;
use xmas_elf::program::{ProgramHeader, Type as ProgramHeaderType};
use xmas_elf::ElfFile;

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

impl<'a> Loader<'a> {
    pub fn new(kernel_data: &'a [u8]) -> LoaderResult<Self> {
        let ptr = &kernel_data[0] as *const u8;
        let kernel_offset = PhysAddr::new(ptr as u64);
        Ok(Loader {
            kernel_offset,
            elf_file: ElfFile::new(kernel_data)?,
        })
    }

    fn map_load_segment(&self, phdr: ProgramHeader, mappings: &mut Mappings) -> LoaderResult<()> {
        let vaddr = VirtAddr::new(phdr.virtual_addr());
        if !is_valid_kernel_addr(vaddr) {
            return Err(LoaderError::InvalidKernelSegmentAddress(vaddr));
        }
        Ok(())
    }

    // Loads the kernel and returns the virtual address of the entry point.
    pub fn load_kernel(&self, mappings: &mut Mappings) -> LoaderResult<VirtAddr> {
        for phdr in self.elf_file.program_iter() {
            if matches!(phdr.get_type()?, ProgramHeaderType::Load) {
                self.map_load_segment(phdr, mappings)?;
            }
        }

        // We'll deal with this later.
        Ok(VirtAddr::new(KERNEL_START))
    }
}
