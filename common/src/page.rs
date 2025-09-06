/* Page table code. This handles exactly the bare minimum needed to get the kernel and boot structures paged into
 * memory. The kernel will probably use a better version of this code, which is why this is separate.
 */

use core::fmt::Display;

use crate::{
    addr::{Address, Page, PhysAddr, PhysFrame, VirtAddr},
    PAGE_SIZE, PHYSADDR_SIZE,
};

#[repr(transparent)]
pub struct PageTableEntry {
    entry: u64,
}

// We should probably use a library instead of writing all these shifts, but whatever.
impl PageTableEntry {
    const PRESENT_IDX: u8 = 0;
    const WRITE_IDX: u8 = 1;
    // This is technically not the page size bit for all levels of the paging structures.
    // Pending a rewrite to use a different type for each level of paging entry, we will just
    // pinky promise not to use it at the wrong level.
    const PAGE_SIZE_IDX: u8 = 7;
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

    pub fn page_size(&self) -> bool {
        self.get_flag(PageTableEntry::PAGE_SIZE_IDX)
    }

    pub fn set_page_size(&mut self, page_size: bool) {
        self.set_flag(page_size, PageTableEntry::PAGE_SIZE_IDX);
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

// Safety: The implementation must return a free, usable frame of memory
pub unsafe trait PageMapAllocator {
    fn alloc(&mut self) -> PhysFrame;
}

// Safety: The implementation of Mapper must return a virtual address mapped
// to the passed physical address.
pub unsafe trait Mapper {
    fn to_table_virt_addr(phys_addr: PhysAddr) -> VirtAddr;
}

const NUM_PAGE_TABLE_ENTRIES: usize = 512;

pub type PageTableEntries = [PageTableEntry; NUM_PAGE_TABLE_ENTRIES];

pub trait PageTable<M: Mapper>: Sized {
    fn entries(&self) -> &PageTableEntries;
    fn entries_mut(&mut self) -> &mut PageTableEntries;
    fn get_entry_idx(addr: VirtAddr) -> usize;

    fn clear(&mut self) {
        for e in self.entries_mut() {
            e.clear();
        }
    }

    fn get_entry(&self, addr: VirtAddr) -> &PageTableEntry {
        self.entries()
            .get(Self::get_entry_idx(addr))
            .expect("Page entry index out of range!")
    }

    fn get_entry_mut(&mut self, addr: VirtAddr) -> &mut PageTableEntry {
        self.entries_mut()
            .get_mut(Self::get_entry_idx(addr))
            .expect("Page entry index out of range!")
    }

    fn alloc_new<'a, A: PageMapAllocator>(allocator: &mut A) -> (&'a mut Self, PhysAddr) {
        let frame = allocator.alloc();

        // From the safety condition on PageMapAllocator, this frame of memory is safe to
        // use as a page table.
        let table = unsafe { Self::from_frame(frame) };
        table.clear();

        (table, frame.base_addr())
    }

    // Safety: addr must point to a frame that contains a valid page table.
    unsafe fn from_frame<'a>(frame: PhysFrame) -> &'a mut Self {
        let virt_addr = M::to_table_virt_addr(frame.base_addr());
        &mut *(virt_addr.as_u64() as *mut Self)
    }
}

pub trait IntermediatePageTable<E: PageTable<M>, M: Mapper>: PageTable<M> {
    fn get<'a>(&'a self, addr: VirtAddr) -> Option<&'a E> {
        // We're masking out 9 bits = 512, so this should always succeed.
        let entry = self.get_entry(addr);

        if entry.present() {
            // Safety: The only way to insert an address into the table is via
            // insert, which always inserts a valid address from FrameAllocator.
            let frame = PhysFrame::from_base_addr(entry.addr());
            unsafe { Some(E::from_frame(frame)) }
        } else {
            None
        }
    }

    fn get_mut<'a>(&'a mut self, addr: VirtAddr) -> Option<&'a mut E> {
        // We're masking out 9 bits = 512, so this should always succeed.
        let entry = self.get_entry_mut(addr);

        if entry.present() {
            // Safety: The only way to insert an address into the table is via
            // insert, which always inserts a valid address from FrameAllocator.
            let frame = PhysFrame::from_base_addr(entry.addr());
            unsafe { Some(E::from_frame(frame)) }
        } else {
            None
        }
    }

    fn insert<'a, A: PageMapAllocator>(
        &'a mut self,
        addr: VirtAddr,
        allocator: &mut A,
    ) -> &'a mut E {
        let allocated = allocator.alloc();

        // Safety: An intermediate page table can fit into the one valid frame `allocated`.
        // The table is cleared to a valid state.
        let reference = unsafe {
            let ptr = allocated.base_u64() as *mut E;
            let reference = &mut *ptr;
            reference.clear();

            reference
        };

        // All indexes are 9 bits, and we have a capacity of 512, so this should always succeed.
        let entry = self.get_entry_mut(addr);
        entry.set_addr(allocated.base_addr());
        entry.set_present(true);
        // Set all intermediate page tables to allow writes - we'll control write access
        // through individual, bottom level page table entries.
        entry.set_write(true);

        reference
    }

    fn get_mut_or_insert<'a, A: PageMapAllocator>(
        &'a mut self,
        addr: VirtAddr,
        allocator: &mut A,
    ) -> &'a mut E {
        // We're masking out 9 bits = 512, so this should always succeed.
        let entry = self.get_entry_mut(addr);

        if entry.present() {
            let frame = PhysFrame::from_base_addr(entry.addr());
            // Safety: The only way to insert an address into the table is via
            // insert, which always inserts a valid address from PageMapAllocator.
            unsafe { E::from_frame(frame) }
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
