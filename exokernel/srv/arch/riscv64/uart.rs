// src/arch/riscv64/uart.rs
//! NS16550A UART驱动

const UART0_BASE: usize = 0x10000000; // QEMU virt

unsafe fn write_reg(offset: usize, val: u8) {
    core::ptr::write_volatile((UART0_BASE + offset) as *mut u8, val);
}

unsafe fn read_reg(offset: usize) -> u8 {
    core::ptr::read_volatile((UART0_BASE + offset) as *const u8)
}

pub unsafe fn init() {
    write_reg(1, 0x00); // 禁用中断
    write_reg(3, 0x80); // 启用DLAB
    write_reg(0, 0x03); // 波特率除数 低字节
    write_reg(1, 0x00); // 波特率除数 高字节
    write_reg(3, 0x03); // 8N1
    write_reg(2, 0xC7); // 启用FIFO
}

pub fn write_byte(byte: u8) {
    unsafe {
        while (read_reg(5) & 0x20) == 0 {}
        write_reg(0, byte);
    }
}
