// src/arch/aarch64/mod.rs
use core::arch::{asm, global_asm};

pub mod boot;
pub mod uart;

pub struct AArch64;

impl super::Architecture for AArch64 {
    fn early_init() {
        unsafe {
            uart::init();
        }
    }

    fn halt() {
        unsafe { asm!("wfi", options(nomem, nostack)); }
    }

    fn enable_interrupts() {
        unsafe { asm!("msr daifclr, #2", options(nomem, nostack)); }
    }

    fn disable_interrupts() {
        unsafe { asm!("msr daifset, #2", options(nomem, nostack)); }
    }

    fn write_serial(byte: u8) {
        uart::write_byte(byte);
    }
}

pub fn halt() { AArch64::halt() }
pub fn enable_interrupts() { AArch64::enable_interrupts() }
pub fn disable_interrupts() { AArch64::disable_interrupts() }
pub fn write_serial(byte: u8) { AArch64::write_serial(byte) }
