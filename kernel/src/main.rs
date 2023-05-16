#![no_std]
#![no_main]

use core::{arch::asm, fmt::Write, panic::PanicInfo, ptr};

use common::{BootInfo, FramebufferInfo, PAGE_SIZE};

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
    let mut ret: u8;
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

pub struct SerialConsole {}

impl core::fmt::Write for SerialConsole {
    fn write_str(&mut self, s: &str) -> core::fmt::Result {
        for c in s.as_bytes() {
            write_serial(*c);
        }

        Ok(())
    }
}

const BPP: usize = 4;

fn clear_framebuffer(fb: &FramebufferInfo) {
    for x in 0..fb.width {
        for y in 0..fb.height {
            let pixel_ptr = fb.address.wrapping_add((y * fb.stride * BPP) + x * BPP);
            unsafe {
                ptr::write_volatile(pixel_ptr, 0);
                ptr::write_volatile(pixel_ptr.add(1), 0);
                ptr::write_volatile(pixel_ptr.add(2), 0);
                ptr::write_volatile(pixel_ptr.add(3), 0);
            }
        }
    }
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

    let mut console = SerialConsole {};

    for region in &*boot_info.mem_regions {
        let ty_code = match region.ty {
            common::RegionType::Usable => 'U',
            common::RegionType::Allocated => 'A',
            common::RegionType::Bootloader => 'B',
        };
        write!(
            console,
            "{}: {} pages ({:#016x} - {:#016x})\r\n",
            ty_code,
            region.pages,
            region.start,
            region.start + (region.pages * PAGE_SIZE)
        )
        .unwrap();
    }

    clear_framebuffer(&boot_info.framebuffer);

    loop {}
}
