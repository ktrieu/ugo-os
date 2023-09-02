use core::fmt::Display;

use common::{PAGE_SIZE, PHYSADDR_SIZE, VIRTADDR_SIZE};

pub fn align_down(addr: u64, align: u64) -> u64 {
    if !align.is_power_of_two() {
        panic!("Cannot align to non power of two alignment {align}!")
    }

    addr & !(align - 1)
}

pub fn align_up(addr: u64, align: u64) -> u64 {
    if is_aligned(addr, align) {
        addr
    } else {
        align_down(addr, align) + align
    }
}

pub fn is_aligned(addr: u64, align: u64) -> bool {
    align_down(addr, align) == addr
}

pub trait Address
where
    Self: Sized + Copy,
{
    fn as_u64(&self) -> u64;
    fn new(addr: u64) -> Self;

    fn align_down(&self, align: u64) -> Self {
        Self::new(align_down(self.as_u64(), align))
    }

    fn align_up(&self, align: u64) -> Self {
        Self::new(align_up(self.as_u64(), align))
    }

    fn is_aligned(&self, align: u64) -> bool {
        is_aligned(self.as_u64(), align)
    }

    fn as_u8_ptr(&self) -> *const u8 {
        self.as_u64() as *const u8
    }

    fn as_u8_ptr_mut(&self) -> *mut u8 {
        self.as_u64() as *mut u8
    }
}

#[derive(PartialEq, PartialOrd, Clone, Copy)]
pub struct PhysAddr(u64);

impl Address for PhysAddr {
    fn as_u64(&self) -> u64 {
        self.0
    }

    fn new(addr: u64) -> Self {
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
}

impl Display for PhysAddr {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        write!(f, "0x{:016x}", self.0)
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
}

impl Address for VirtAddr {
    fn as_u64(&self) -> u64 {
        self.0
    }

    fn new(addr: u64) -> Self {
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
}

impl Display for VirtAddr {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        write!(f, "0x{:016x}", self.0)
    }
}

// A virtual page/physical frame.
pub trait Page
where
    Self: Sized + Copy,
{
    type A: Address;

    fn from_base_addr(addr: Self::A) -> Self;
    fn base_addr(&self) -> Self::A;

    fn from_containing_addr(addr: Self::A) -> Self {
        let aligned = addr.align_down(PAGE_SIZE);

        Self::from_base_addr(aligned)
    }

    fn increment(&self, n: u64) -> Self {
        Self::from_base_u64(self.base_addr().as_u64() + (n * PAGE_SIZE))
    }

    fn decrement(&self, n: u64) -> Self {
        Self::from_base_u64(self.base_addr().as_u64() - (n * PAGE_SIZE))
    }

    fn next(&self) -> Self {
        self.increment(1)
    }

    fn range_inclusive(start: Self, end: Self) -> PageRange<Self> {
        PageRange::new(start, end.next())
    }

    fn range_exclusive(start: Self, end: Self) -> PageRange<Self> {
        PageRange::new(start, end)
    }

    fn range_length(start: Self, n: u64) -> PageRange<Self> {
        let end = start.increment(n);
        Self::range_exclusive(start, end)
    }

    fn range_inclusive_u64(start: u64, end: u64) -> PageRange<Self> {
        let start = Self::from_containing_u64(start);
        let end = Self::from_containing_u64(end);

        return Self::range_inclusive(start, end);
    }

    fn range_exclusive_u64(start: u64, end: u64) -> PageRange<Self> {
        let start = Self::from_containing_u64(start);
        let end = Self::from_containing_u64(end);

        return Self::range_exclusive(start, end);
    }

    fn from_base_u64(addr: u64) -> Self {
        Self::from_base_addr(Self::A::new(addr))
    }

    fn from_containing_u64(addr: u64) -> Self {
        Self::from_containing_addr(Self::A::new(addr))
    }

    fn as_u8_ptr(&self) -> *const u8 {
        self.base_addr().as_u8_ptr()
    }

    fn as_u8_ptr_mut(&self) -> *mut u8 {
        self.base_addr().as_u8_ptr_mut()
    }

    fn base_u64(&self) -> u64 {
        self.base_addr().as_u64()
    }
}

#[derive(Clone, Copy)]
pub struct PageRange<P: Page + Copy> {
    // Inclusive lower bound.
    start: P,
    // Exclusive upper bound.
    end: P,
}

impl<P: Page> PageRange<P> {
    pub fn new(start: P, end: P) -> Self {
        Self { start, end }
    }

    pub fn first(&self) -> P {
        self.start
    }

    pub fn last(&self) -> P {
        self.end.decrement(1)
    }

    pub fn len(&self) -> u64 {
        (self.end.base_u64() - self.start.base_u64()) / PAGE_SIZE
    }

    pub fn contains(&self, page: P) -> bool {
        page.base_u64() >= self.start.base_u64() && page.base_u64() < self.end.base_u64()
    }

    pub fn iter(&self) -> PageRangeIter<P> {
        PageRangeIter {
            next: self.start,
            range: *self,
        }
    }
}

#[derive(Clone, Copy)]
pub struct PageRangeIter<P: Page> {
    next: P,
    range: PageRange<P>,
}

impl<P: Page> Iterator for PageRangeIter<P> {
    type Item = P;

    fn next(&mut self) -> Option<Self::Item> {
        let ret = self.next;

        if self.range.contains(ret) {
            self.next = ret.next();
            Some(ret)
        } else {
            None
        }
    }
}

#[derive(Clone, Copy, PartialEq, PartialOrd)]
pub struct PhysFrame(PhysAddr);

impl PhysFrame {
    pub fn to_virt_page(&self, offset: u64) -> VirtPage {
        VirtPage::from_base_u64(self.0.as_u64() + offset)
    }
}

impl Page for PhysFrame {
    type A = PhysAddr;

    fn from_base_addr(addr: PhysAddr) -> Self {
        assert!(
            addr.is_aligned(PAGE_SIZE),
            "Physical address {:016x} is not aligned to a page boundary!",
            addr.as_u64()
        );
        PhysFrame(addr)
    }

    fn base_addr(&self) -> PhysAddr {
        self.0
    }
}

impl Display for PhysFrame {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        write!(f, "Frame: {}", self.0)
    }
}

#[derive(Clone, Copy, PartialEq, PartialOrd)]
pub struct VirtPage(VirtAddr);

impl Page for VirtPage {
    type A = VirtAddr;

    fn from_base_addr(addr: VirtAddr) -> Self {
        assert!(
            addr.is_aligned(PAGE_SIZE),
            "Virtual address {:016x} is not aligned to a page boundary!",
            addr.as_u64()
        );

        VirtPage(addr)
    }

    fn base_addr(&self) -> VirtAddr {
        self.0
    }
}

impl Display for VirtPage {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        // Add a second space here so the address is aligned with the Frame Display impl. I'm sure we'll be
        // comparing these two often.
        write!(f, "Page:  {}", self.0)
    }
}
