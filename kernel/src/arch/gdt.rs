use core::arch::asm;

use bilge::prelude::*;
use conquer_once::spin::OnceCell;
use spin::Mutex;

#[bitsize(1)]
#[derive(FromBits)]
pub enum DescriptorType {
    System = 0,
    CodeStack = 1,
}

#[bitsize(2)]
#[derive(FromBits)]
pub enum PrivilegeLevel {
    Kernel = 0,
    Level1,
    Level2,
    User = 3,
}

#[bitsize(1)]
#[derive(FromBits)]
pub enum SegmentType {
    Code,
    Stack,
}

#[bitsize(64)]
pub struct GdtEntry {
    limit_low: u16,
    address_low: u24,
    accessed: bool,
    // Allows read for code segments, and write for data segments.
    read_write: bool,
    expand_down: bool,
    ty: SegmentType,
    descriptor_type: DescriptorType,
    privilege_level: PrivilegeLevel,
    present: bool,
    limit_high: u4,
    available: bool,
    is_64_bit: bool,
    use_32_bit_addresses: bool,
    is_limit_granular: bool,
    address_high: u8,
}

impl Default for GdtEntry {
    fn default() -> Self {
        GdtEntry::new(
            // Set base/limit to zero. These are ignored in x64 anyway.
            0,
            u24::new(0),
            // The system will set accessed, and it's recommended that we set it to false.
            false,
            // This allows code segments to be read, and data segments to be written. Default this to true.
            false,
            // Allows segments to "expand down". We set these to full size anyway, so set this to false.
            false,
            // Calling code will set these, so just set these to any value.
            SegmentType::Code,
            DescriptorType::CodeStack,
            PrivilegeLevel::Kernel,
            // Just set present to false, the rest doesn't really matter.
            false,
            // Set base/limit to zero.
            u4::new(0),
            // This is available for our use. The value doesn't matter.
            false,
            // Set every segment to 64-bit.
            true,
            // Set code/stack segments to use 32 bit addresses. Probably meaningless in x64?
            true,
            // Set the segment limit to not use a 4096 byte granularity. Surely this doesn't matter either.
            false,
            // Set the limit register to zero.
            0,
        )
    }
}

impl GdtEntry {
    pub fn new_kernel_code_segment() -> Self {
        let mut entry = Self::default();

        entry.set_descriptor_type(DescriptorType::CodeStack);
        entry.set_ty(SegmentType::Code);
        entry.set_privilege_level(PrivilegeLevel::Kernel);

        entry
    }

    pub fn new_kernel_stack_segment() -> Self {
        let mut entry = Self::default();

        entry.set_descriptor_type(DescriptorType::CodeStack);
        entry.set_ty(SegmentType::Stack);
        entry.set_privilege_level(PrivilegeLevel::Kernel);

        entry
    }
}

const GDT_LENGTH: usize = 3;

type GdtStatic = OnceCell<Mutex<[GdtEntry; GDT_LENGTH]>>;
static GDT: GdtStatic = OnceCell::uninit();

#[repr(packed)]
// We read from these via the LGDT instruction, but Rust doesn't know that.
#[allow(dead_code)]
struct GdtBase {
    length: u16,
    address: u64,
}

pub fn initialize_gdt() {
    // Avoid double initializing.
    assert!(!GDT.is_initialized());

    GDT.init_once(|| {
        // Important: the first entry needs to be null.
        let entries = [
            GdtEntry::default(),
            GdtEntry::new_kernel_code_segment(),
            GdtEntry::new_kernel_stack_segment(),
        ];

        Mutex::new(entries)
    });

    let base = GdtBase {
        length: GDT_LENGTH as u16 * 32,
        address: (&GDT as *const GdtStatic) as u64,
    };

    unsafe {
        asm!(
            "lgdt [{ptr}]",
            ptr = in(reg) &base
        );
    }
}
