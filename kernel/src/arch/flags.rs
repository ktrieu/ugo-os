use core::arch::asm;

use bilge::prelude::*;

use super::PrivilegeLevel;

#[bitsize(64)]
#[derive(FromBits)]
pub struct RFlags {
    pub carry: bool,
    pub reserved: bool,
    pub parity: bool,
    pub reserved: bool,
    pub aux_carry: bool,
    pub reserved: bool,
    pub zero: bool,
    pub sign: bool,
    pub trap: bool,
    pub interrupts_enabled: bool,
    pub direction: bool,
    pub overflow: bool,
    pub io_privilege_level: PrivilegeLevel,
    pub nested_task: bool,
    pub nec_mode_flag: bool,
    pub resume: bool,
    pub virtual_8086: bool,
    pub alignment_check: bool,
    pub virtual_interrupts_enabled: bool,
    pub virtual_interrupt_pending: bool,
    pub cpuid_available: bool,
    pub reserved: u8,
    pub via_aes_key_sched_loaded: bool,
    pub via_alternate_instruction_set: bool,
    pub reserved: u32,
}

pub fn get_rflags() -> RFlags {
    let mut flags = RFlags { value: 0 };
    unsafe {
        asm!(
            "pushfq
             pop rax",
            out("rax") flags.value
        );
    }
    flags
}
