use bilge::prelude::*;

use super::PrivilegeLevel;

#[bitsize(4)]
#[derive(TryFromBits)]
pub enum GateType {
    Interrupt = 0b1110,
    Trap = 0b1111,
}

#[bitsize(1)]
#[derive(FromBits)]
pub enum SelectorTarget {
    Global = 0,
    Local = 1,
}

#[bitsize(16)]
#[derive(FromBits)]
pub struct SegmentSelector {
    privilege_level: PrivilegeLevel,
    target: SelectorTarget,
    index: u13,
}

pub const IDT_ENTRY_SIZE: u16 = 128;

#[bitsize(128)]
#[derive(Clone, Copy)]
pub struct IdtEntry {
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
