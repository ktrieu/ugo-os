use core::arch::asm;

use common::{
    addr::{is_aligned, Address, Page, PhysAddr, PhysFrame, VirtAddr, VirtPage},
    page::{IntermediatePageTable, Mapper, PageTable, PageTableEntries, PageTableEntry},
    PAGE_SIZE,
};

use crate::kmem::phys::PhysFrameAllocator;

struct DirectMapper {}

// Safety: In the kernel environment, all physical memory will be directly mapped
// (at PHYSMEM_OFFSET)
unsafe impl Mapper for DirectMapper {
    fn to_table_virt_addr(phys_addr: PhysAddr) -> VirtAddr {
        return phys_addr.as_direct_mapped();
    }
}

pub struct KernelPageMapLevel1(PageTableEntries);

impl PageTable<DirectMapper> for KernelPageMapLevel1 {
    fn entries(&self) -> &PageTableEntries {
        &self.0
    }

    fn entries_mut(&mut self) -> &mut PageTableEntries {
        &mut self.0
    }

    fn get_entry_idx(addr: VirtAddr) -> usize {
        addr.get_page_table_idx().try_into().unwrap()
    }
}

impl IntermediatePageTable<KernelPageMapLevel1, DirectMapper> for KernelPageMapLevel1 {}

pub struct KernelPageMapLevel2(PageTableEntries);

impl PageTable<DirectMapper> for KernelPageMapLevel2 {
    fn entries(&self) -> &PageTableEntries {
        &self.0
    }

    fn entries_mut(&mut self) -> &mut PageTableEntries {
        &mut self.0
    }

    fn get_entry_idx(addr: VirtAddr) -> usize {
        addr.get_page_dir_idx().try_into().unwrap()
    }
}

impl IntermediatePageTable<KernelPageMapLevel1, DirectMapper> for KernelPageMapLevel2 {}

pub struct KernelPageMapLevel3(PageTableEntries);

impl PageTable<DirectMapper> for KernelPageMapLevel3 {
    fn entries(&self) -> &PageTableEntries {
        &self.0
    }

    fn entries_mut(&mut self) -> &mut PageTableEntries {
        &mut self.0
    }

    fn get_entry_idx(addr: VirtAddr) -> usize {
        addr.get_page_dir_ptr_idx().try_into().unwrap()
    }
}

impl IntermediatePageTable<KernelPageMapLevel2, DirectMapper> for KernelPageMapLevel3 {}

pub struct KernelPageMapLevel4(PageTableEntries);

impl PageTable<DirectMapper> for KernelPageMapLevel4 {
    fn entries(&self) -> &PageTableEntries {
        &self.0
    }

    fn entries_mut(&mut self) -> &mut PageTableEntries {
        &mut self.0
    }

    fn get_entry_idx(addr: VirtAddr) -> usize {
        addr.get_page_map_l4_idx().try_into().unwrap()
    }
}
impl IntermediatePageTable<KernelPageMapLevel3, DirectMapper> for KernelPageMapLevel4 {}

pub struct KernelPageTables<'a> {
    level_4_table: &'a mut KernelPageMapLevel4,
    _level_4_phys_addr: PhysAddr,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MappingType {
    DataRw,
}

impl MappingType {
    pub fn apply(&self, entry: &mut PageTableEntry) {
        match self {
            MappingType::DataRw => {
                entry.set_no_exec(true);
                entry.set_write(true);
                entry.set_present(true);
            }
        }
    }
}

impl<'a> KernelPageTables<'a> {
    pub fn new() -> Self {
        // Grab the physical address out of CR3 - our page tables are already setup.
        let mut cr3: u64 = 0;
        unsafe {
            asm!(
                "mov {}, cr3",
                out(reg) cr3
            )
        };

        // This had better be page-aligned.
        assert!(is_aligned(cr3, PAGE_SIZE));
        let addr = PhysAddr::new(cr3);

        // Safety: This pointer comes from CR3 so it must be a valid page table.
        let table = unsafe { KernelPageMapLevel4::from_frame(PhysFrame::from_base_addr(addr)) };

        Self {
            level_4_table: table,
            _level_4_phys_addr: PhysAddr::new(cr3),
        }
    }

    pub fn get_entry(&self, addr: VirtAddr) -> Option<PageTableEntry> {
        let level_3_map = self.level_4_table.get(addr)?;
        // We directly map the physical memory into level 3 huge pages. Check for this
        // and don't traverse farther if the page size flag is set.
        let level_3_entry = level_3_map.get_entry(addr);
        if level_3_entry.present() && level_3_entry.page_size() {
            return Some(*level_3_entry);
        }

        let level_2_map = level_3_map.get(addr)?;
        let level_1_map = level_2_map.get(addr)?;

        Some(*level_1_map.get_entry(addr))
    }

    pub fn alloc_and_map_page(
        &mut self,
        page: VirtPage,
        ty: MappingType,
        allocator: &mut PhysFrameAllocator,
    ) {
        let addr = page.base_addr();
        let level_3_map = self.level_4_table.get_mut_or_insert(addr, allocator);
        // We directly map the physical memory into level 3 huge pages. Check for this
        // and don't traverse farther if the page size flag is set.
        let level_3_entry = level_3_map.get_entry(addr);
        if level_3_entry.present() && level_3_entry.page_size() {
            panic!("tried to map area mapped as huge-page")
        }

        let level_2_map = level_3_map.get_mut_or_insert(addr, allocator);
        let level_1_map = level_2_map.get_mut_or_insert(addr, allocator);

        let entry = level_1_map.get_entry_mut(addr);

        let backing_frame = allocator
            .alloc_frame()
            .expect("memory should be available for backing frame");

        entry.set_addr(backing_frame.base_addr());
        ty.apply(entry);
    }
}
