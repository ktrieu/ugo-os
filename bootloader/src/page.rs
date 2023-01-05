/* Page table code. This handles exactly the bare minimum needed to get the kernel and boot structures paged into
 * memory. The kernel will probably use a better version of this code, which is why this is separate.
 */

use core::fmt::Display;

use common::{PAGE_SIZE, PHYSADDR_SIZE};

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

    pub fn align_down(self, align: u64) -> PhysAddr {
        if !align.is_power_of_two() {
            panic!("Cannot align to non power of two alignment {align}!")
        }

        PhysAddr(self.0 & !(align - 1))
    }

    pub fn align_up(self, align: u64) -> PhysAddr {
        let aligned_down = self.align_down(align);

        PhysAddr(aligned_down.0 + align)
    }

    pub fn as_u64(self) -> u64 {
        self.0
    }
}

pub struct PhysFrame(PhysAddr);

impl PhysFrame {
    pub fn from_phys_addr(addr: PhysAddr) -> PhysFrame {
        let base_addr = addr.align_down(PAGE_SIZE);

        PhysFrame(base_addr)
    }

    pub fn base_addr(self) -> PhysAddr {
        self.0
    }
}

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

    pub fn new() -> PageTableEntry {
        PageTableEntry { entry: 0 }
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

#[repr(C, align(4096))]
struct PageMapLevel4 {
    entries: [PageTableEntry; NUM_PAGE_TABLE_ENTRIES],
}

#[repr(C, align(4096))]
struct PageDirPointerTable {
    entries: [PageTableEntry; NUM_PAGE_TABLE_ENTRIES],
}

#[repr(C, align(4096))]
struct PageDir {
    entries: [PageTableEntry; NUM_PAGE_TABLE_ENTRIES],
}

#[repr(C, align(4096))]
struct PageTable {
    entries: [PageTableEntry; NUM_PAGE_TABLE_ENTRIES],
}
