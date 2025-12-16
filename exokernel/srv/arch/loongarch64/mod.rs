// src/arch/loongarch64/mod.rs
use core::arch::{asm, global_asm};

pub mod boot;
pub mod uart;

pub struct LoongArch64;

impl super::Architecture for LoongArch64 {
    fn early_init() {
        unsafe {
            uart::init();
        }
    }

    fn halt() {
        unsafe { asm!("idle 0", options(nomem, nostack)); }
    }

    fn enable_interrupts() {
        unsafe {
            asm!("csrwr $t0, 0x0", // CSR_CRMD
            out("$t0") _,
            options(nomem, nostack));
        }
    }

    fn disable_interrupts() {
        unsafe {
            asm!("csrwr $zero, 0x0",
            options(nomem, nostack));
        }
    }

    fn write_serial(byte: u8) {
        uart::write_byte(byte);
    }
}

pub fn halt() { LoongArch64::halt() }
pub fn enable_interrupts() { LoongArch64::enable_interrupts() }
pub fn disable_interrupts() { LoongArch64::disable_interrupts() }
pub fn write_serial(byte: u8) { LoongArch64::write_serial(byte) }
