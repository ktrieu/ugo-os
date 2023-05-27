use core::{arch::asm, marker::PhantomData};

pub trait PortValue: Sized + Copy {
    fn asm_in(port: Port<Self>) -> Self;
    fn asm_out(&self, port: Port<Self>);
}

impl PortValue for u8 {
    fn asm_in(port: Port<Self>) -> Self {
        let value: u8;

        unsafe {
            asm!(
                "in {value}, dx",
                in("dx") port.0,
                value = out(reg_byte) value
            )
        };

        value
    }

    fn asm_out(&self, port: Port<Self>) {
        unsafe {
            asm!(
                "out dx, {value}",
                in("dx") port.0,
                value = in(reg_byte) *self
            )
        };
    }
}

impl PortValue for u16 {
    fn asm_in(port: Port<Self>) -> Self {
        let value: u16;

        unsafe {
            asm!(
                "in {0:x}, dx",
                out(reg) value,
                in("dx") port.0,
            )
        };

        value
    }

    fn asm_out(&self, port: Port<Self>) {
        unsafe {
            asm!(
                "out dx, {0:x}",
                in(reg) *self,
                in("dx") port.0,
            )
        };
    }
}

impl PortValue for u32 {
    fn asm_in(port: Port<Self>) -> Self {
        let value: u32;

        unsafe {
            asm!(
                "in {0:e}, dx",
                out(reg) value,
                in("dx") port.0,
            )
        };

        value
    }

    fn asm_out(&self, port: Port<Self>) {
        unsafe {
            asm!(
                "out dx, {0:e}",
                in(reg) *self,
                in("dx") port.0,
            )
        };
    }
}

#[derive(Clone, Copy)]
pub struct Port<V: PortValue>(pub u16, PhantomData<V>);

impl<V: PortValue> Port<V> {
    pub const fn new(address: u16) -> Self {
        Port(address, PhantomData)
    }

    pub fn write(&self, value: V) {
        value.asm_out(*self);
    }

    pub fn read(&self) -> V {
        V::asm_in(*self)
    }
}
