// src/arch/riscv64/boot.rs
use core::arch::global_asm;

global_asm!(
    r#"
    .section .text.boot
    .global _start

_start:
    // a0 = hartid
    // a1 = dtb物理地址

    // 保存DTB地址
    la t0, dtb_ptr
    sd a1, (t0)

    // 设置栈
    la sp, boot_stack_top

    // 清理BSS
    la t0, __bss_start
    la t1, __bss_end
.clear_bss:
    bgeu t0, t1, .bss_done
    sd zero, (t0)
    addi t0, t0, 8
    j .clear_bss
.bss_done:

    // 跳转到Rust
    call kernel_main

.hang:
    wfi
    j .hang

    .section .bss
    .align 16
boot_stack_bottom:
    .space 0x10000
boot_stack_top:

    .section .data
dtb_ptr:
    .dword 0
    "#
);

extern "C" {
    static dtb_ptr: u64;
}

pub unsafe fn get_boot_info() -> *const u8 {
    dtb_ptr as *const u8
}
