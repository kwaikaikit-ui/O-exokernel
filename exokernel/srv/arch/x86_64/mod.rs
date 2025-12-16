// src/arch/x86_64/mod.rs
use core::arch::{asm, global_asm};

pub mod boot;
pub mod serial;

pub struct X86_64;

impl super::Architecture for X86_64 {
    fn early_init() {
        unsafe {
            gdt_init();
            serial::init();
        }
    }

    fn halt() {
        unsafe { asm!("hlt", options(nomem, nostack)); }
    }

    fn enable_interrupts() {
        unsafe { asm!("sti", options(nomem, nostack)); }
    }

    fn disable_interrupts() {
        unsafe { asm!("cli", options(nomem, nostack)); }
    }

    fn write_serial(byte: u8) {
        serial::write_byte(byte);
    }
}

pub fn halt() { X86_64::halt() }
pub fn enable_interrupts() { X86_64::enable_interrupts() }
pub fn disable_interrupts() { X86_64::disable_interrupts() }
pub fn write_serial(byte: u8) { X86_64::write_serial(byte) }

// GDT结构
#[repr(C, packed)]
struct GdtEntry {
    limit_low: u16,
    base_low: u16,
    base_mid: u8,
    access: u8,
    granularity: u8,
    base_high: u8,
}

impl GdtEntry {
    const fn new(access: u8, flags: u8) -> Self {
        Self {
            limit_low: 0xFFFF,
            base_low: 0,
            base_mid: 0,
            access,
            granularity: 0xCF | (flags << 4),
            base_high: 0,
        }
    }

    const fn null() -> Self {
        Self {
            limit_low: 0, base_low: 0, base_mid: 0,
            access: 0, granularity: 0, base_high: 0,
        }
    }
}

#[repr(C, packed)]
struct GdtPointer {
    limit: u16,
    base: u64,
}

static mut GDT: [GdtEntry; 5] = [
    GdtEntry::null(),
    GdtEntry::new(0x9A, 0xA), // 内核代码段
    GdtEntry::new(0x92, 0xC), // 内核数据段
    GdtEntry::new(0xFA, 0xA), // 用户代码段
    GdtEntry::new(0xF2, 0xC), // 用户数据段
];

unsafe fn gdt_init() {
    let gdt_ptr = GdtPointer {
        limit: (core::mem::size_of_val(&GDT) - 1) as u16,
        base: GDT.as_ptr() as u64,
    };

    asm!(
    "lgdt [{}]",
    "mov ax, 0x10",
    "mov ds, ax",
    "mov es, ax",
    "mov fs, ax",
    "mov gs, ax",
    "mov ss, ax",
    "push 0x08",
    "lea rax, [rip + 2f]",
    "push rax",
    "retfq",
    "2:",
    in(reg) &gdt_ptr,
    out("rax") _,
    out("ax") _,
    );
}
