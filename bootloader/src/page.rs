use common::{
    addr::{Address, PhysAddr, VirtAddr},
    page::{IntermediatePageTable, Mapper, PageTable, PageTableEntries},
};

pub struct IdentityMapper {}

// Safety: During boot, the entire physical memory space is identity mapped.
unsafe impl Mapper for IdentityMapper {
    fn to_table_virt_addr(phys_addr: PhysAddr) -> VirtAddr {
        return VirtAddr::new(phys_addr.as_u64());
    }
}

pub struct BootPageMapLevel1(PageTableEntries);

impl PageTable<IdentityMapper> for BootPageMapLevel1 {
    fn _entries(&self) -> &PageTableEntries {
        &self.0
    }

    fn entries_mut(&mut self) -> &mut PageTableEntries {
        &mut self.0
    }

    fn get_entry_idx(addr: VirtAddr) -> usize {
        addr.get_page_table_idx().try_into().unwrap()
    }
}

impl IntermediatePageTable<BootPageMapLevel1, IdentityMapper> for BootPageMapLevel1 {}

pub struct BootPageMapLevel2(PageTableEntries);

impl PageTable<IdentityMapper> for BootPageMapLevel2 {
    fn _entries(&self) -> &PageTableEntries {
        &self.0
    }

    fn entries_mut(&mut self) -> &mut PageTableEntries {
        &mut self.0
    }

    fn get_entry_idx(addr: VirtAddr) -> usize {
        addr.get_page_dir_idx().try_into().unwrap()
    }
}

impl IntermediatePageTable<BootPageMapLevel1, IdentityMapper> for BootPageMapLevel2 {}

pub struct BootPageMapLevel3(PageTableEntries);

impl PageTable<IdentityMapper> for BootPageMapLevel3 {
    fn _entries(&self) -> &PageTableEntries {
        &self.0
    }

    fn entries_mut(&mut self) -> &mut PageTableEntries {
        &mut self.0
    }

    fn get_entry_idx(addr: VirtAddr) -> usize {
        addr.get_page_dir_ptr_idx().try_into().unwrap()
    }
}

impl IntermediatePageTable<BootPageMapLevel2, IdentityMapper> for BootPageMapLevel3 {}

pub struct BootPageMapLevel4(PageTableEntries);

impl PageTable<IdentityMapper> for BootPageMapLevel4 {
    fn _entries(&self) -> &PageTableEntries {
        &self.0
    }

    fn entries_mut(&mut self) -> &mut PageTableEntries {
        &mut self.0
    }

    fn get_entry_idx(addr: VirtAddr) -> usize {
        addr.get_page_map_l4_idx().try_into().unwrap()
    }
}
impl IntermediatePageTable<BootPageMapLevel3, IdentityMapper> for BootPageMapLevel4 {}
