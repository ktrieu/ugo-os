use core::arch::asm;

use super::flags::get_rflags;

pub mod handler;
pub mod idt;

// Safety: You must ensure interrupts are eventually re-enabled.
pub fn disable_interrupts() {
    unsafe { asm!("cli") };
}

pub fn enable_interrupts() {
    unsafe { asm!("sti") };
}

pub fn are_interrupts_enabled() -> bool {
    get_rflags().interrupts_enabled()
}

pub fn with_interrupts_disabled<R, F: FnOnce() -> R>(f: F) -> R {
    let was_enabled = are_interrupts_enabled();

    if was_enabled {
        disable_interrupts();
    }

    let ret = f();

    if was_enabled {
        enable_interrupts();
    }

    ret
}
