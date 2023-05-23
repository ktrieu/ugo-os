#[repr(packed)]
pub struct ExceptionFrame {
    instruction_pointer: u64,
    code_segment: u64,
    flags: u64,
    stack_pointer: u64,
    stack_segment: u64,
}
