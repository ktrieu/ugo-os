#![no_std]
#![no_main]

use core::{arch::asm, fmt::Write, panic::PanicInfo};

use common::BootInfo;
use framebuffer::TextFramebuffer;

mod framebuffer;

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
    let mut ret: u8;
    unsafe {
        asm!(
            "in al, dx",
            out("al") ret,
            in("dx") port,
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

pub struct SerialConsole {}

impl core::fmt::Write for SerialConsole {
    fn write_str(&mut self, s: &str) -> core::fmt::Result {
        for c in s.as_bytes() {
            write_serial(*c);
        }

        Ok(())
    }
}

#[panic_handler]
fn panic(info: &PanicInfo) -> ! {
    let mut c = SerialConsole {};
    write!(c, "{}", info).unwrap();

    loop {}
}

#[no_mangle]
pub extern "C" fn _start(boot_info: &'static mut BootInfo) -> ! {
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

    let mut text_fb = TextFramebuffer::new(&boot_info.framebuffer);
    text_fb.clear();

    for c in ('A'..'Z').cycle() {
        for _ in 0..text_fb.line_length() {
            text_fb.write_char(c);
        }
    }

    loop {}
}
