use bilge::prelude::*;
use spin::Mutex;

use crate::arch::{gdt::SegmentSelector, PrivilegeLevel};

use super::handler::ExceptionFrame;

#[bitsize(4)]
#[derive(TryFromBits)]
pub enum GateType {
    Interrupt = 0b1110,
    Trap = 0b1111,
}

#[bitsize(128)]
#[derive(Clone, Copy)]
struct IdtEntryBase {
    offset_low: u16,
    selector: SegmentSelector,
    ist_offset: u3,
    reserved: u5,
    gate_type: GateType,
    zero: bool,
    privilege_level: PrivilegeLevel,
    present: bool,
    offset_high: u48,
    reserved: u32,
}

impl IdtEntryBase {
    const LENGTH_BYTES: u16 = 16;

    pub const fn default() -> Self {
        Self { value: 0 }
    }

    fn set_address(&mut self, address: u64) {
        // Inconveniently, the address is in two fields.
        let address_low_mask = !(u64::MAX << 16);
        let address_low = address & address_low_mask;
        let address_high = address >> 16;

        self.set_offset_low(address_low.try_into().unwrap());
        self.set_offset_high(u48::new(address_high));
    }

    fn set_handler(&mut self, address: u64) {
        self.set_address(address);
        self.set_gate_type(GateType::Interrupt);
        self.set_present(true);
    }
}

type IdtHandler = fn(ExceptionFrame);
type IdtHandlerWithErrorCode = fn(ExceptionFrame, error_code: u64);

// Wrapper types so we can ensure we register the correct handlers at compile time.
#[repr(transparent)]
#[derive(Clone, Copy)]
pub struct IdtEntryWithErrorCode(IdtEntryBase);

impl IdtEntryWithErrorCode {
    pub const fn default() -> Self {
        Self(IdtEntryBase::default())
    }

    // Safety: handler must point to a function defined with the x86-interrupt calling convention.
    pub fn set_handler(&mut self, handler: IdtHandlerWithErrorCode) {
        self.0
            .set_handler(&handler as *const IdtHandlerWithErrorCode as u64);
    }
}

#[repr(transparent)]
#[derive(Clone, Copy)]
pub struct IdtEntry(IdtEntryBase);

impl IdtEntry {
    pub const fn default() -> Self {
        Self(IdtEntryBase::default())
    }

    pub fn set_handler(&mut self, handler: IdtHandler) {
        self.0.set_handler(&handler as *const IdtHandler as u64);
    }
}

#[repr(packed)]
pub struct Idt {
    div_zero: IdtEntry,
    debug: IdtEntry,
    nmi: IdtEntry,
    breakpoint: IdtEntry,
    overflow: IdtEntry,
    bound: IdtEntry,
    invalid_opcode: IdtEntry,
    device_not_available: IdtEntry,
    double_fault: IdtEntryWithErrorCode,
    coprocessor_overrun: IdtEntry,
    invalid_tss: IdtEntryWithErrorCode,
    segment_not_present: IdtEntryWithErrorCode,
    stack_fault: IdtEntryWithErrorCode,
    general_protection: IdtEntryWithErrorCode,
    page_fault: IdtEntryWithErrorCode,
    floating_point_error: IdtEntry,
    alignment_check: IdtEntryWithErrorCode,
    machine_check: IdtEntry,
    simd_floating_point: IdtEntry,
    virtualization: IdtEntry,
    // Interrupts 32 - 255 are user defined and have no error codes.
    user_defined: [IdtEntry; Self::NUM_USER_DEFINED],
}

impl Idt {
    const NUM_ENTRIES: usize = 255;
    const USER_DEFINED_START: usize = 32;

    const NUM_USER_DEFINED: usize = Self::NUM_ENTRIES - Self::USER_DEFINED_START;

    pub const fn default() -> Self {
        Self {
            div_zero: IdtEntry::default(),
            debug: IdtEntry::default(),
            nmi: IdtEntry::default(),
            breakpoint: IdtEntry::default(),
            overflow: IdtEntry::default(),
            bound: IdtEntry::default(),
            invalid_opcode: IdtEntry::default(),
            device_not_available: IdtEntry::default(),
            double_fault: IdtEntryWithErrorCode::default(),
            coprocessor_overrun: IdtEntry::default(),
            invalid_tss: IdtEntryWithErrorCode::default(),
            segment_not_present: IdtEntryWithErrorCode::default(),
            stack_fault: IdtEntryWithErrorCode::default(),
            general_protection: IdtEntryWithErrorCode::default(),
            page_fault: IdtEntryWithErrorCode::default(),
            floating_point_error: IdtEntry::default(),
            alignment_check: IdtEntryWithErrorCode::default(),
            machine_check: IdtEntry::default(),
            simd_floating_point: IdtEntry::default(),
            virtualization: IdtEntry::default(),
            user_defined: [IdtEntry::default(); Self::NUM_USER_DEFINED],
        }
    }
}

pub static IDT: Mutex<Idt> = Mutex::new(Idt::default());

extern "x86-interrupt" fn div_handler() {}
