use bilge::prelude::*;

pub mod gdt;
pub mod interrupts;

#[bitsize(2)]
#[derive(Debug, Clone, Copy, FromBits)]
pub enum PrivilegeLevel {
    Kernel = 0,
    Level1 = 1,
    Level2 = 2,
    User = 3,
}
