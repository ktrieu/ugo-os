use core::fmt::Display;

use common::{PAGE_SIZE, PHYSADDR_SIZE, VIRTADDR_SIZE};

fn align_down(addr: u64, align: u64) -> u64 {
    if !align.is_power_of_two() {
        panic!("Cannot align to non power of two alignment {align}!")
    }

    addr & !(align - 1)
}

fn align_up(addr: u64, align: u64) -> u64 {
    align_down(addr, align) + align
}

fn is_aligned(addr: u64, align: u64) -> bool {
    align_down(addr, align) == addr
}

#[derive(PartialEq, PartialOrd, Clone, Copy)]
pub struct PhysAddr(u64);

impl PhysAddr {
    pub fn new(addr: u64) -> PhysAddr {
        let upper_bits = addr >> PHYSADDR_SIZE;
        if upper_bits != 0 {
            panic!(
                "Upper {} bits of physical address {:#X} must be 0!",
                64 - PHYSADDR_SIZE,
                addr
            );
        }

        PhysAddr(addr)
    }

    pub fn align_down(&self, align: u64) -> PhysAddr {
        PhysAddr(align_down(self.0, align))
    }

    pub fn align_up(&self, align: u64) -> PhysAddr {
        PhysAddr(align_up(self.0, align))
    }

    pub fn is_aligned(&self, align: u64) -> bool {
        is_aligned(self.0, align)
    }

    pub fn as_u64(self) -> u64 {
        self.0
    }
}

impl Display for PhysAddr {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        write!(f, "0x{:016x}", self.0)
    }
}

pub struct FrameIterExclusive {
    curr: PhysFrame,
    end: PhysFrame,
}

impl Iterator for FrameIterExclusive {
    type Item = PhysFrame;

    fn next(&mut self) -> Option<Self::Item> {
        let ret = self.curr;
        if ret.base_addr() > self.end.base_addr() {
            None
        } else {
            self.curr = ret.next_frame();
            Some(ret)
        }
    }
}

pub struct FrameIterInclusive {
    curr: PhysFrame,
    end: PhysFrame,
}

impl Iterator for FrameIterInclusive {
    type Item = PhysFrame;

    fn next(&mut self) -> Option<Self::Item> {
        let ret = self.curr;
        if ret.base_addr() >= self.end.base_addr() {
            None
        } else {
            self.curr = ret.next_frame();
            Some(ret)
        }
    }
}

#[derive(Clone, Copy)]
pub struct PhysFrame(PhysAddr);

impl PhysFrame {
    pub fn from_containing_phys_addr(addr: PhysAddr) -> PhysFrame {
        let base_addr = addr.align_down(PAGE_SIZE);

        PhysFrame(base_addr)
    }

    pub fn from_base_phys_addr(addr: PhysAddr) -> PhysFrame {
        assert!(
            addr.is_aligned(PAGE_SIZE),
            "Provided unaligned base address for PhysFrame!"
        );

        PhysFrame(addr)
    }

    pub fn add_frames(&self, frames: u64) -> PhysFrame {
        PhysFrame::from_base_u64(self.base_addr().as_u64() + frames * PAGE_SIZE)
    }

    pub fn next_frame(&self) -> PhysFrame {
        self.add_frames(1)
    }

    pub fn range_inclusive(&self, end: PhysFrame) -> FrameIterInclusive {
        FrameIterInclusive { curr: *self, end }
    }

    pub fn range_exclusive(&self, end: PhysFrame) -> FrameIterExclusive {
        FrameIterExclusive { curr: *self, end }
    }

    pub fn from_base_u64(addr: u64) -> PhysFrame {
        PhysFrame::from_base_phys_addr(PhysAddr::new(addr))
    }

    pub fn from_containing_u64(addr: u64) -> PhysFrame {
        PhysFrame::from_containing_phys_addr(PhysAddr::new(addr))
    }

    pub fn base_addr(&self) -> PhysAddr {
        self.0
    }

    pub fn to_virt_page(&self, offset: u64) -> VirtPage {
        assert!(is_aligned(offset, PAGE_SIZE));
        VirtPage::from_base_u64(self.0.as_u64() + offset)
    }
}

impl Display for PhysFrame {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        write!(f, "Frame: {}", self.0)
    }
}

#[derive(PartialEq, PartialOrd, Clone, Copy)]
pub struct VirtAddr(u64);

impl VirtAddr {
    // The number of bits in the index for a page table entry (of any level).
    const ENTRY_IDX_SIZE: u8 = 9;
    // The number of bits in the final offset into a page.
    const PAGE_OFFSET_SIZE: u8 = 12;
    // The mask for the index into a page table.
    const PAGE_TABLE_IDX_MASK: u64 =
        2_u64.pow(VirtAddr::ENTRY_IDX_SIZE as u32) - 1 << VirtAddr::PAGE_OFFSET_SIZE;
    // The mask for the index into a page directory.
    const PAGE_DIR_IDX_MASK: u64 = VirtAddr::PAGE_TABLE_IDX_MASK << VirtAddr::ENTRY_IDX_SIZE;
    // The mask for the index into a page directory pointer table.
    const PAGE_DIR_PTR_IDX_MASK: u64 = VirtAddr::PAGE_DIR_IDX_MASK << VirtAddr::ENTRY_IDX_SIZE;
    // The mask for the index into a page map level 4.
    const PAGE_MAP_L4_IDX_MASK: u64 = VirtAddr::PAGE_DIR_PTR_IDX_MASK << VirtAddr::ENTRY_IDX_SIZE;

    pub fn new(addr: u64) -> VirtAddr {
        // Check if the address is in canonical form (sign-extended) first.
        let upper_bit = addr & (1 << VIRTADDR_SIZE - 1) > 0;

        let required_sign_extension = if upper_bit {
            2_u64.pow(64 - VIRTADDR_SIZE as u32) - 1
        } else {
            0
        };

        let sign_extension = addr >> VIRTADDR_SIZE;
        assert!(
            sign_extension == required_sign_extension,
            "Address {:#016x} was not a canonical virtual address!",
            addr
        );

        VirtAddr(addr)
    }

    pub fn get_page_table_idx(&self) -> u64 {
        (self.0 & VirtAddr::PAGE_TABLE_IDX_MASK) >> VirtAddr::PAGE_OFFSET_SIZE
    }

    pub fn get_page_dir_idx(&self) -> u64 {
        (self.0 & VirtAddr::PAGE_DIR_IDX_MASK)
            >> VirtAddr::ENTRY_IDX_SIZE + VirtAddr::PAGE_OFFSET_SIZE
    }

    pub fn get_page_dir_ptr_idx(&self) -> u64 {
        (self.0 & VirtAddr::PAGE_DIR_PTR_IDX_MASK)
            >> VirtAddr::ENTRY_IDX_SIZE * 2 + VirtAddr::PAGE_OFFSET_SIZE
    }

    pub fn get_page_map_l4_idx(&self) -> u64 {
        (self.0 & VirtAddr::PAGE_MAP_L4_IDX_MASK)
            >> VirtAddr::ENTRY_IDX_SIZE * 3 + VirtAddr::PAGE_OFFSET_SIZE
    }

    pub fn as_u64(&self) -> u64 {
        self.0
    }

    pub fn align_up(&self, align: u64) -> VirtAddr {
        VirtAddr(align_up(self.0, align))
    }

    pub fn align_down(&self, align: u64) -> VirtAddr {
        VirtAddr(align_down(self.0, align))
    }

    pub fn is_aligned(&self, align: u64) -> bool {
        is_aligned(self.0, align)
    }
}

impl Display for VirtAddr {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        write!(f, "0x{:016x}", self.0)
    }
}

pub struct PageIterExclusive {
    curr: VirtPage,
    end: VirtPage,
}

impl Iterator for PageIterExclusive {
    type Item = VirtPage;

    fn next(&mut self) -> Option<Self::Item> {
        let ret = self.curr;
        if ret.base_addr() > self.end.base_addr() {
            None
        } else {
            self.curr = ret.next_page();
            Some(ret)
        }
    }
}

pub struct PageIterInclusive {
    curr: VirtPage,
    end: VirtPage,
}

impl Iterator for PageIterInclusive {
    type Item = VirtPage;

    fn next(&mut self) -> Option<Self::Item> {
        let ret = self.curr;
        if ret.base_addr() >= self.end.base_addr() {
            None
        } else {
            self.curr = ret.next_page();
            Some(ret)
        }
    }
}

#[derive(Clone, Copy)]
pub struct VirtPage(VirtAddr);

impl VirtPage {
    pub fn from_containing_addr(addr: VirtAddr) -> VirtPage {
        VirtPage(addr.align_down(PAGE_SIZE))
    }

    pub fn from_base_addr(addr: VirtAddr) -> VirtPage {
        assert!(
            addr.is_aligned(PAGE_SIZE),
            "Virtual address {:016x} is not aligned to a page boundary!",
            addr.as_u64()
        );

        VirtPage(addr)
    }

    pub fn from_base_u64(base: u64) -> VirtPage {
        VirtPage::from_base_addr(VirtAddr::new(base))
    }

    pub fn base_addr(&self) -> VirtAddr {
        self.0
    }

    pub fn add_pages(&self, pages: u64) -> VirtPage {
        VirtPage::from_base_u64(self.base_addr().as_u64() + pages * PAGE_SIZE)
    }

    pub fn next_page(&self) -> VirtPage {
        self.add_pages(1)
    }

    pub fn range_inclusive(&self, end: VirtPage) -> PageIterInclusive {
        PageIterInclusive { curr: *self, end }
    }

    pub fn range_exclusive(&self, end: VirtPage) -> PageIterExclusive {
        PageIterExclusive { curr: *self, end }
    }

    pub fn from_containing_u64(containing: u64) -> VirtPage {
        VirtPage::from_containing_addr(VirtAddr::new(containing))
    }
}

impl Display for VirtPage {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        // Add a second space here so the address is aligned with the Frame Display impl. I'm sure we'll be
        // comparing these two often.
        write!(f, "Page:  {}", self.0)
    }
}
