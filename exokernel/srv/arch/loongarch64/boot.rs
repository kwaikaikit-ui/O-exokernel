// src/arch/loongarch64/boot.rs
use core::arch::global_asm;

global_asm!(
    r#"
    .section .text.boot
    .global _start

_start:
    // a0 = bootinfo指针
    la.global $t0, boot_info_ptr
    st.d $a0, $t0, 0

    // 设置栈
    la.global $sp, boot_stack_top

    // 清理BSS
    la.global $t0, __bss_start
    la.global $t1, __bss_end
.clear_bss:
    bgeu $t0, $t1, .bss_done
    st.d $zero, $t0, 0
    addi.d $t0, $t0, 8
    b .clear_bss
.bss_done:

    // 跳转到Rust
    bl kernel_main

.hang:
    idle 0
    b .hang

    .section .bss
    .align 16
boot_stack_bottom:
    .space 0x10000
boot_stack_top:

    .section .data
boot_info_ptr:
    .dword 0
    "#
);

extern "C" {
    static boot_info_ptr: u64;
}

pub unsafe fn get_boot_info() -> *const u8 {
    boot_info_ptr as *const u8
}
