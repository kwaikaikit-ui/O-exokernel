// src/arch/aarch64/uart.rs
//! PL011 UART驱动

const UART0_BASE: usize = 0x09000000; // QEMU virt

unsafe fn write_reg(offset: usize, val: u32) {
    core::ptr::write_volatile((UART0_BASE + offset) as *mut u32, val);
}

unsafe fn read_reg(offset: usize) -> u32 {
    core::ptr::read_volatile((UART0_BASE + offset) as *const u32)
}

pub unsafe fn init() {
    write_reg(0x30, 0); // 禁用UART
    write_reg(0x24, 0x70); // 设置波特率
    write_reg(0x28, 0);
    write_reg(0x2C, 0x60); // 8N1
    write_reg(0x30, 0x301); // 启用UART
}

pub fn write_byte(byte: u8) {
    unsafe {
        while (read_reg(0x18) & 0x20) != 0 {}
        write_reg(0x00, byte as u32);
    }
}

