/* Page table code. This handles exactly the bare minimum needed to get the kernel and boot structures paged into
 * memory. The kernel will probably use a better version of this code, which is why this is separate.
 */

use core::fmt::Display;

use common::{PAGE_SIZE, PHYSADDR_SIZE};

use crate::{
    addr::{PhysAddr, VirtAddr},
    frame::FrameAllocator,
};

#[repr(transparent)]
pub struct PageTableEntry {
    entry: u64,
}

// We should probably use a library instead of writing all these shifts, but whatever.
impl PageTableEntry {
    const PRESENT_IDX: u8 = 0;
    const WRITE_IDX: u8 = 1;
    const NO_EXEC_IDX: u8 = 63;
    const ADDR_IDX: u8 = 12;
    const ADDR_SIZE: u8 = PHYSADDR_SIZE - PageTableEntry::ADDR_IDX;

    fn get_flag(&self, index: u8) -> bool {
        self.entry & (1 << index) > 0
    }

    fn set_flag(&mut self, flag: bool, index: u8) {
        self.entry = self.entry & !(1 << index) | ((flag as u64) << index);
    }

    pub fn present(&self) -> bool {
        self.get_flag(PageTableEntry::PRESENT_IDX)
    }

    pub fn set_present(&mut self, present: bool) {
        self.set_flag(present, PageTableEntry::PRESENT_IDX);
    }

    pub fn write(&self) -> bool {
        self.get_flag(PageTableEntry::WRITE_IDX)
    }

    pub fn set_write(&mut self, rw: bool) {
        self.set_flag(rw, PageTableEntry::WRITE_IDX);
    }

    pub fn no_exec(&self) -> bool {
        self.get_flag(PageTableEntry::NO_EXEC_IDX)
    }

    pub fn set_no_exec(&mut self, no_exec: bool) {
        self.set_flag(no_exec, PageTableEntry::NO_EXEC_IDX);
    }

    pub fn addr(&self) -> PhysAddr {
        let addr_mask =
            (2_u64.pow(PageTableEntry::ADDR_SIZE as u32) - 1) << PageTableEntry::ADDR_IDX;

        PhysAddr::new(self.entry & addr_mask)
    }

    pub fn set_addr(&mut self, addr: PhysAddr) {
        let addr_mask =
            (2_u64.pow(PageTableEntry::ADDR_SIZE as u32) - 1) << PageTableEntry::ADDR_IDX;
        // Mask the addr bits to zero
        self.entry &= !addr_mask;
        // And OR in our address, aligned down to the page boundary
        let aligned = addr.align_down(PAGE_SIZE);
        self.entry |= aligned.as_u64();
    }

    pub fn clear(&mut self) {
        self.entry = 0;
    }
}

impl Display for PageTableEntry {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        /*
         * For now, only display the fields we care about
         * - No exec
         * - Address
         * - Write
         * - Present
         */
        f.write_fmt(format_args!(
            "{:b} {:0width$b} {:b} {:b}",
            self.no_exec() as u8,
            self.addr().as_u64(),
            self.write() as u8,
            self.present() as u8,
            width = PageTableEntry::ADDR_SIZE as usize
        ))
    }
}

const NUM_PAGE_TABLE_ENTRIES: usize = 512;
type PageTableEntries = [PageTableEntry; NUM_PAGE_TABLE_ENTRIES];

pub trait PageTable: Sized {
    fn entries(&self) -> &PageTableEntries;
    fn entries_mut(&mut self) -> &mut PageTableEntries;
    fn get_entry_idx(addr: VirtAddr) -> usize;

    fn clear(&mut self) {
        for e in self.entries_mut() {
            e.clear();
        }
    }

    fn alloc_new<'a>(allocator: &mut FrameAllocator) -> (&'a mut Self, PhysAddr) {
        let frame = allocator.alloc_frame();

        // Safety: An intermediate page table can fit into exactly one physical frame, which is returned for use
        // by the FrameAllocator. The page table is cleared to a valid state.
        unsafe {
            let ptr = frame.base_addr().as_u64() as *mut Self;
            let reference = &mut *ptr;
            reference.clear();
            (reference, frame.base_addr())
        }
    }

    fn get_entry_mut(&mut self, addr: VirtAddr) -> &mut PageTableEntry {
        self.entries_mut()
            .get_mut(Self::get_entry_idx(addr))
            .expect("Page entry index out of range!")
    }

    unsafe fn from_phys_addr<'a>(addr: PhysAddr) -> &'a mut Self {
        &mut *(addr.as_u64() as *mut Self)
    }
}

pub trait IntermediatePageTable<E: PageTable>: PageTable {
    fn insert<'a>(&'a mut self, addr: VirtAddr, allocator: &mut FrameAllocator) -> &'a mut E {
        let (new_table, new_addr) = E::alloc_new(allocator);
        // All indexes are 9 bits, and we have a capacity of 512, so this should always succeed.
        let entry = self.get_entry_mut(addr);
        entry.set_addr(new_addr);
        entry.set_present(true);
        // Set all intermediate page tables to allow writes - we'll control write access
        // through individual, bottom level page table entries.
        entry.set_write(true);

        new_table
    }

    fn get_mut<'a>(&'a mut self, addr: VirtAddr) -> Option<&'a mut E> {
        // We're masking out 9 bits = 512, so this should always succeed.
        let entry = self.get_entry_mut(addr);

        if entry.present() {
            // Safety: The only way to insert an address into the table is via
            // insert, which always inserts a valid address from FrameAllocator.
            unsafe { Some(E::from_phys_addr(entry.addr())) }
        } else {
            None
        }
    }

    fn get_mut_or_insert(&mut self, addr: VirtAddr, allocator: &mut FrameAllocator) -> &mut E {
        // We're masking out 9 bits = 512, so this should always succeed.
        let entry = self.get_entry_mut(addr);

        if entry.present() {
            // Safety: The only way to insert an address into the table is via
            // insert, which always inserts a valid address from FrameAllocator.
            unsafe { E::from_phys_addr(entry.addr()) }
        } else {
            let (new_table, new_addr) = E::alloc_new(allocator);
            entry.set_addr(new_addr);
            entry.set_present(true);
            // Set all intermediate page tables to allow writes - we'll control write access
            // through individual, bottom level page table entries.
            entry.set_write(true);

            new_table
        }
    }
}

#[repr(transparent)]
pub struct PageMapLevel4 {
    entries: [PageTableEntry; NUM_PAGE_TABLE_ENTRIES],
}

impl PageTable for PageMapLevel4 {
    fn entries(&self) -> &PageTableEntries {
        &self.entries
    }

    fn entries_mut(&mut self) -> &mut PageTableEntries {
        &mut self.entries
    }

    fn get_entry_idx(addr: VirtAddr) -> usize {
        addr.get_page_map_l4_idx() as usize
    }
}

impl IntermediatePageTable<PageMapLevel3> for PageMapLevel4 {}

#[repr(transparent)]
pub struct PageMapLevel3 {
    entries: [PageTableEntry; NUM_PAGE_TABLE_ENTRIES],
}

impl PageTable for PageMapLevel3 {
    fn entries(&self) -> &PageTableEntries {
        &self.entries
    }

    fn entries_mut(&mut self) -> &mut PageTableEntries {
        &mut self.entries
    }

    fn get_entry_idx(addr: VirtAddr) -> usize {
        addr.get_page_dir_ptr_idx() as usize
    }
}

impl IntermediatePageTable<PageMapLevel2> for PageMapLevel3 {}

#[repr(transparent)]
pub struct PageMapLevel2 {
    entries: [PageTableEntry; NUM_PAGE_TABLE_ENTRIES],
}

impl PageTable for PageMapLevel2 {
    fn entries(&self) -> &PageTableEntries {
        &self.entries
    }

    fn entries_mut(&mut self) -> &mut PageTableEntries {
        &mut self.entries
    }

    fn get_entry_idx(addr: VirtAddr) -> usize {
        addr.get_page_dir_idx() as usize
    }
}

impl IntermediatePageTable<PageMapLevel1> for PageMapLevel2 {}

#[repr(transparent)]
pub struct PageMapLevel1 {
    entries: [PageTableEntry; NUM_PAGE_TABLE_ENTRIES],
}

impl PageTable for PageMapLevel1 {
    fn entries(&self) -> &PageTableEntries {
        &self.entries
    }

    fn entries_mut(&mut self) -> &mut PageTableEntries {
        &mut self.entries
    }

    fn get_entry_idx(addr: VirtAddr) -> usize {
        addr.get_page_table_idx() as usize
    }
}
