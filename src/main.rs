#![no_main]
#![no_std]
#![feature(abi_efiapi)]

use core::fmt::Write;

use uefi::prelude::*;

#[entry]
fn uefi_main(_handle: Handle, mut system_table: SystemTable<Boot>) -> Status {
    uefi_services::init(&mut system_table).unwrap();

    system_table
        .stdout()
        .write_str("Hello from ugoOS!")
        .unwrap();

    loop {}
}
