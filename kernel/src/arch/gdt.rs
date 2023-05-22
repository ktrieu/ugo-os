use core::arch::asm;

use bilge::prelude::*;
use spin::Mutex;

use super::PrivilegeLevel;

#[bitsize(1)]
#[derive(FromBits)]
pub enum DescriptorType {
    System = 0,
    // This is a code/data segment for application use.
    NonSystem = 1,
}

#[bitsize(1)]
#[derive(FromBits)]
pub enum SegmentType {
    Data = 0,
    Code = 1,
}

#[bitsize(64)]
#[derive(Clone, Copy, FromBits)]
pub struct GdtEntry {
    limit_low: u16,
    address_low: u24,
    accessed: bool,
    // Allows read for code segments, and write for data segments.
    read_write: bool,
    dc_flag: bool,
    ty: SegmentType,
    descriptor_type: DescriptorType,
    privilege_level: PrivilegeLevel,
    present: bool,
    limit_high: u4,
    available: bool,
    is_64_bit_code: bool,
    use_32_bit_addresses: bool,
    is_limit_page_granular: bool,
    address_high: u8,
}

impl GdtEntry {
    const LENGTH_BYTES: usize = 8;

    // I would like to implement this as the Default trait, but I need this to be const as well.
    pub const fn default() -> Self {
        GdtEntry { value: 0 }
    }

    pub fn new_64_bit_segment() -> Self {
        let mut entry = Self::default();

        entry.set_present(true);

        entry
    }

    pub fn new_kernel_code_segment() -> Self {
        let mut entry = Self::new_64_bit_segment();

        entry.set_descriptor_type(DescriptorType::NonSystem);
        entry.set_ty(SegmentType::Code);
        entry.set_privilege_level(PrivilegeLevel::Kernel);
        entry.set_is_64_bit_code(true);

        entry
    }

    pub fn new_kernel_data_segment() -> Self {
        let mut entry = Self::new_64_bit_segment();

        entry.set_descriptor_type(DescriptorType::NonSystem);
        entry.set_ty(SegmentType::Data);
        entry.set_read_write(true);
        entry.set_privilege_level(PrivilegeLevel::Kernel);

        entry
    }

    pub fn new_user_code_segment() -> Self {
        let mut entry = Self::new_64_bit_segment();

        entry.set_descriptor_type(DescriptorType::NonSystem);
        entry.set_ty(SegmentType::Code);
        entry.set_privilege_level(PrivilegeLevel::User);
        entry.set_is_64_bit_code(true);

        entry
    }

    pub fn new_user_data_segment() -> Self {
        let mut entry = Self::new_64_bit_segment();

        entry.set_descriptor_type(DescriptorType::NonSystem);
        entry.set_ty(SegmentType::Data);
        entry.set_read_write(true);
        entry.set_privilege_level(PrivilegeLevel::User);

        entry
    }
}

// The LGDT instruction reads these fields, but Rust doesn't know that.
#[allow(dead_code)]
#[repr(packed)]
struct GdtBase {
    length: u16,
    address: u64,
}

#[repr(transparent)]
pub struct Gdt {
    entries: [GdtEntry; Gdt::LENGTH],
}

impl Gdt {
    // Four user/kernel code/data segments, plus the requisite null first entry.
    const LENGTH: usize = 5;

    const KERNEL_CODE_SEGMENT_IDX: usize = 1;
    const KERNEL_DATA_SEGMENT_IDX: usize = 2;
    const USER_CODE_SEGMENT_IDX: usize = 3;
    const USER_DATA_SEGMENT_IDX: usize = 4;

    pub fn initialize(&mut self) {
        self.entries[Self::KERNEL_CODE_SEGMENT_IDX] = GdtEntry::new_kernel_code_segment();
        self.entries[Self::KERNEL_DATA_SEGMENT_IDX] = GdtEntry::new_kernel_data_segment();
        self.entries[Self::USER_CODE_SEGMENT_IDX] = GdtEntry::new_user_code_segment();
        self.entries[Self::USER_DATA_SEGMENT_IDX] = GdtEntry::new_user_data_segment();
    }

    // Safety: The GDT this points to must be initialized with valid kernel code/data segments before calling this function.
    pub unsafe fn activate(&self) {
        let length: u16 = (Self::LENGTH * GdtEntry::LENGTH_BYTES - 1)
            .try_into()
            .expect("GDT length couldn't fit into a u16");
        let base = GdtBase {
            address: &self.entries as *const GdtEntry as u64,
            length,
        };

        asm!(
            "
            lgdt [{base}]
            mov rax, 0x08
            push rax
            lea rax, [rip + 2f]
            push rax
            retfq
            2:
            mov ax, 0x10
            mov ds, ax
            mov es, ax
            mov fs, ax
            mov gs, ax
            mov ss, ax
            ",
            base = in(reg) &base
        )
    }
}

static GDT: Mutex<Gdt> = Mutex::new(Gdt {
    entries: [GdtEntry::default(); Gdt::LENGTH],
});

pub fn initialize_gdt() {
    GDT.lock().initialize();

    // Safety: We have initialized the GDT.
    unsafe { GDT.lock().activate() }
}
