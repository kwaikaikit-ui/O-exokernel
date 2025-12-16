// src/arch/loongarch64/uart.rs
//! LoongArch UART驱动（基于NS16550）

const UART0_BASE: usize = 0x1FE001E0;

unsafe fn write_reg(offset: usize, val: u8) {
    core::ptr::write_volatile((UART0_BASE + offset) as *mut u8, val);
}

unsafe fn read_reg(offset: usize) -> u8 {
    core::ptr::read_volatile((UART0_BASE + offset) as *const u8)
}

pub unsafe fn init() {
    write_reg(1, 0x00);
    write_reg(3, 0x80);
    write_reg(0, 0x01);
    write_reg(1, 0x00);
    write_reg(3, 0x03);
    write_reg(2, 0xC7);
}

pub fn write_byte(byte: u8) {
    unsafe {
        while (read_reg(5) & 0x20) == 0 {}
        write_reg(0, byte);
    }
}
