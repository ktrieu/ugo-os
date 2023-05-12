#![no_std]
#![no_main]

use core::{arch::asm, panic::PanicInfo};

#[panic_handler]
fn panic(_info: &PanicInfo) -> ! {
    loop {}
}

// Real fast: let's try and get something on the serial port. This code is Very Bad.
fn outb(port: u16, byte: u8) {
    unsafe {
        asm!(
            "out dx, al",
            in("dx") port,
            in("al") byte
        );
    }
}

fn inb(port: u16) -> u8 {
    let mut ret: u8 = 0;
    unsafe {
        asm!(
            "in {ret}, dx",
            in("dx") port,
            ret = out(reg_byte) ret,
        );
    }

    ret
}

const COM1: u16 = 0x3f8;

fn is_transmit_empty() -> bool {
    inb(COM1 + 5) & 0x20 == 0
}

fn write_serial(c: u8) {
    while is_transmit_empty() {}

    outb(COM1, c);
}

const MSG: &'static [u8] = b"HELLO FROM UGO-OS";

#[no_mangle]
pub extern "C" fn _start() -> ! {
    outb(COM1 + 1, 0x00); // Disable all interrupts
    outb(COM1 + 3, 0x80); // Enable DLAB (set baud rate divisor)
    outb(COM1 + 0, 0x03); // Set divisor to 3 (lo byte) 38400 baud
    outb(COM1 + 1, 0x00); //                  (hi byte)
    outb(COM1 + 3, 0x03); // 8 bits, no parity, one stop bit
    outb(COM1 + 2, 0xC7); // Enable FIFO, clear them, with 14-byte threshold
    outb(COM1 + 4, 0x0B); // IRQs enabled, RTS/DSR set
    outb(COM1 + 4, 0x1E); // Set in loopback mode, test the serial chip
    outb(COM1 + 0, 0xAE);
    outb(COM1 + 4, 0x0F);

    for c in MSG {
        write_serial(*c);
    }

    loop {}
}
