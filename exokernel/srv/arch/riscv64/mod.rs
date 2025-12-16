// src/arch/riscv64/mod.rs
use core::arch::{asm, global_asm};

pub mod boot;
pub mod uart;

pub struct RiscV64;

impl super::Architecture for RiscV64 {
    fn early_init() {
        unsafe {
            uart::init();
        }
    }

    fn halt() {
        unsafe { asm!("wfi", options(nomem, nostack)); }
    }

    fn enable_interrupts() {
        unsafe { asm!("csrsi sstatus, 0x2", options(nomem, nostack)); }
    }

    fn disable_interrupts() {
        unsafe { asm!("csrci sstatus, 0x2", options(nomem, nostack)); }
    }

    fn write_serial(byte: u8) {
        uart::write_byte(byte);
    }
}

pub fn halt() { RiscV64::halt() }
pub fn enable_interrupts() { RiscV64::enable_interrupts() }
pub fn disable_interrupts() { RiscV64::disable_interrupts() }
pub fn write_serial(byte: u8) { RiscV64::write_serial(byte) }
