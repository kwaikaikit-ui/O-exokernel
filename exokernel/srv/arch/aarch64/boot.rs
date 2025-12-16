// src/arch/aarch64/boot.rs
use core::arch::global_asm;

global_asm!(
    r#"
    .section .text.boot
    .global _start

_start:
    // x0 = dtb物理地址
    ldr x19, =dtb_ptr
    str x0, [x19]

    // 关闭MMU/缓存
    mrs x1, sctlr_el1
    bic x1, x1, #0x1
    bic x1, x1, #0x4
    bic x1, x1, #0x1000
    msr sctlr_el1, x1
    isb

    // 设置栈
    ldr x1, =boot_stack_top
    mov sp, x1

    // 清理BSS
    ldr x1, =__bss_start
    ldr x2, =__bss_end
.clear_bss:
    cmp x1, x2
    b.ge .bss_done
    str xzr, [
    bl kernel_main

.hang:
    wfi
    b .hang

    .section .bss
    .align 16
boot_stack_bottom:
    .space 0x10000
boot_stack_top:

    .section .data
dtb_ptr:
    .quad 0
    "#
);

extern "C" {
    static dtb_ptr: u64;
}

pub unsafe fn get_boot_info() -> *const u8 {
    dtb_ptr as *const u8
}
