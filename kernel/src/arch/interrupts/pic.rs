use crate::{arch::io_port::Port, sync::InterruptSafeSpinlock};

use super::{
    handler::ExceptionFrame,
    idt::{add_user_defined_handler, Idt},
};

#[derive(PartialEq)]
pub enum PicType {
    Master,
    Slave,
}

#[repr(u8)]
#[derive(Clone, Copy)]
#[allow(dead_code)]
pub enum IRQCode {
    IRQ0 = 0,
    IRQ1 = 1,
    IRQ2 = 2,
    IRQ3 = 3,
    IRQ4 = 4,
    IRQ5 = 5,
    IRQ6 = 6,
    IRQ7 = 7,
    IRQ8 = 8,
    IRQ9 = 9,
    IRQ10 = 10,
    IRQ11 = 11,
    IRQ12 = 12,
    IRQ13 = 13,
    IRQ14 = 14,
    IRQ15 = 15,
}

impl IRQCode {
    pub fn source(&self) -> PicType {
        if *self as u8 <= 7 {
            PicType::Master
        } else {
            PicType::Slave
        }
    }

    pub fn index(&self) -> u8 {
        *self as u8
    }

    // Returns the "local index" of the interrupt. That is, an index that is always
    // between 0-7.
    pub fn local_index(&self) -> u8 {
        match self.source() {
            PicType::Master => self.index(),
            PicType::Slave => self.index() - Pic::NUM_INTERRUPTS,
        }
    }
}

pub struct Pic {
    command_port: Port<u8>,
    data_port: Port<u8>,
    interrupt_offset: u8,
    ty: PicType,
}

impl Pic {
    const NUM_INTERRUPTS: u8 = 8;

    const CMD_INIT: u8 = 0x11;
    const CMD_INIT_ICW4: u8 = 0x01;

    // These encode that the slave is on IRQ line 2.
    const SLAVE_IRQ_LINE: u8 = 2;
    const CMD_MASTER: u8 = 1 << Self::SLAVE_IRQ_LINE;
    const CMD_SLAVE: u8 = Self::SLAVE_IRQ_LINE;

    // This sets it to 8086 mode, which everyone does.
    const CMD_ICW4: u8 = 0x01;

    const CMD_EOI: u8 = 0x20;

    const WAIT_PORT: Port<u8> = Port::new(0x80);

    // Wait for the PIC to process commands. Apparently writing to port 0x80
    // creates enough of a wait, and we can't really do anything else without
    // interrupts working.
    pub fn io_port_wait() {
        Self::WAIT_PORT.write(0);
    }

    pub const fn new(ty: PicType, port_base: u16, interrupt_offset: u8) -> Self {
        Pic {
            command_port: Port::new(port_base),
            data_port: Port::new(port_base + 1),
            interrupt_offset,
            ty,
        }
    }

    pub fn initialize(&self) {
        // Save the IRQ masks.
        let irq_masks = self.data_port.read();

        // Send the init sequence.

        // First, the init byte OR'ed with the option enabling the fourth init word.
        self.command_port
            .write(Self::CMD_INIT | Self::CMD_INIT_ICW4);
        Self::io_port_wait();
        // Write the interrupt offset.
        self.data_port.write(self.interrupt_offset);
        Self::io_port_wait();
        // Inform the master which port the slave is on, and tell the slave which port it's been cascaded to.
        match self.ty {
            PicType::Master => self.data_port.write(Self::CMD_MASTER),
            PicType::Slave => self.data_port.write(Self::CMD_SLAVE),
        };
        Self::io_port_wait();
        // And set to 8086 mode, whatever that means.
        self.data_port.write(Self::CMD_ICW4);
        Self::io_port_wait();

        // Restore the saved IRQ masks.
        self.data_port.write(irq_masks);
    }

    pub fn enable_interrupt(&self, index: u8) {
        let mut mask = self.data_port.read();
        mask = mask & !(1 << index);

        self.data_port.write(mask);
    }

    pub fn signal_eoi(&self) {
        self.command_port.write(Self::CMD_EOI);
    }
}

pub struct CascadedPics {
    master: Pic,
    slave: Pic,
}

impl CascadedPics {
    const MASTER_INTERRUPT_OFFSET: u8 = Idt::USER_DEFINED_START as u8;
    const SLAVE_INTERRUPT_OFFSET: u8 = Self::MASTER_INTERRUPT_OFFSET + Pic::NUM_INTERRUPTS;

    const MASTER_PORT: u16 = 0x20;
    const SLAVE_PORT: u16 = 0xA0;

    pub const fn new() -> Self {
        let master = Pic::new(
            PicType::Master,
            Self::MASTER_PORT,
            Self::MASTER_INTERRUPT_OFFSET,
        );

        let slave = Pic::new(
            PicType::Slave,
            Self::SLAVE_PORT,
            Self::SLAVE_INTERRUPT_OFFSET,
        );

        Self { master, slave }
    }

    pub fn initialize(&self) {
        self.master.initialize();
        self.slave.initialize();
    }

    pub fn get_idt_offset(&self, irq: IRQCode) -> u16 {
        match irq.source() {
            PicType::Master => (irq.local_index() + self.master.interrupt_offset).into(),
            PicType::Slave => (irq.local_index() + self.slave.interrupt_offset).into(),
        }
    }

    pub fn enable_interrupt(&self, irq: IRQCode) {
        match irq.source() {
            PicType::Master => self.master.enable_interrupt(irq.local_index()),
            PicType::Slave => self.slave.enable_interrupt(irq.local_index()),
        }
    }

    pub fn signal_eoi(&self, irq: IRQCode) {
        // We need to signal BOTH the master and slave for a slave interrupt,
        // so allow fallthrough here.
        if irq.source() == PicType::Slave {
            self.slave.signal_eoi();
        }

        self.master.signal_eoi();
    }
}

const TIMER_INTERRUPT: IRQCode = IRQCode::IRQ0;
const KEYBOARD_INTERRUPT: IRQCode = IRQCode::IRQ1;

pub extern "x86-interrupt" fn keyboard_handler(_frame: ExceptionFrame) {
    let scancode = Port::<u8>::new(0x60).read();
    kprintln!("KEYBOARD PRESSED! - {:02x}", scancode);
    PIC.lock().signal_eoi(KEYBOARD_INTERRUPT);
}

pub extern "x86-interrupt" fn timer_handler(_frame: ExceptionFrame) {
    // kprintln!("TIMER INTERRUPT!");
    PIC.lock().signal_eoi(TIMER_INTERRUPT);
}

pub static PIC: InterruptSafeSpinlock<CascadedPics> =
    InterruptSafeSpinlock::new(CascadedPics::new());

pub fn initialize_pic() {
    PIC.lock().initialize();

    let pic = PIC.lock();
    pic.enable_interrupt(KEYBOARD_INTERRUPT);
    pic.enable_interrupt(TIMER_INTERRUPT);
    pic.get_idt_offset(KEYBOARD_INTERRUPT);
    // Safety: keyboard/timer_handler are declared with x86-interrupt.s
    unsafe {
        add_user_defined_handler(pic.get_idt_offset(KEYBOARD_INTERRUPT), keyboard_handler);
        add_user_defined_handler(pic.get_idt_offset(TIMER_INTERRUPT), timer_handler);
    }
}
