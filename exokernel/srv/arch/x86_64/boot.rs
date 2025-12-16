// src/arch/x86_64/boot.rs
use core::arch::global_asm;

global_asm!(
    r#"
    .section .multiboot2
    .align 8
multiboot2_header:
    .long 0xe85250d6
    .long 0
    .long multiboot2_header_end - multiboot2_header
    .long -(0xe85250d6 + 0 + (multiboot2_header_end - multiboot2_header))

    .align 8
    .short 1, 0
    .long 20
    .long 6
    .long 8
    .long 10

    .align 8
    .short 0, 0
    .long 8
multiboot2_header_end:

    .section .text.boot
    .global _start
    .code32

_start:
    cli

    mov [multiboot2_magic - KERNEL_VMA], eax
    mov [multiboot2_info - KERNEL_VMA], ebx

    mov esp, (boot_stack_top - KERNEL_VMA)

    pushfd
    pop eax
    mov ecx, eax
    xor eax, 1 << 21
    push eax
    popfd
    pushfd
    pop eax
    push ecx
    popfd
    xor eax, ecx
    jz .no_cpuid

    mov eax, 0x80000000
    cpuid
    cmp eax, 0x80000001
    jb .no_long_mode

    mov eax, 0x80000001
    cpuid
    test edx, 1 << 29
    jz .no_long_mode

    mov eax, (pdpt - KERNEL_VMA)
    or eax, 3
    mov [(pml4 - KERNEL_VMA)], eax
    mov [(pml4 - KERNEL_VMA) + 511*8], eax

    mov eax, (pd - KERNEL_VMA)
    or eax, 3
    mov [(pdpt - KERNEL_VMA)], eax

    mov edi, (pd - KERNEL_VMA)
    mov eax, 0x83
    mov ecx, 512
.map_pd:
    mov [edi], eax
    add eax, 0x200000
    add edi, 8
    loop .map_pd

    mov eax, cr4
    or eax, 1 << 5
    mov cr4, eax

    mov eax, (pml4 - KERNEL_VMA)
    mov cr3, eax

    mov ecx, 0xC0000080
    rdmsr
    or eax, 1 << 8
    wrmsr

    mov eax, cr0
    or eax, 1 << 31
    mov cr0, eax

    lgdt [(gdt64_ptr - KERNEL_VMA)]

    jmp 0x08:.long_mode

.code64
.long_mode:
    mov ax, 0x10
    mov ds, ax
    mov es, ax
    mov fs, ax
    mov gs, ax
    mov ss, ax

    mov rsp, boot_stack_top

    mov rdi, __bss_start
    mov rcx, __bss_end
    sub rcx, rdi
    xor rax, rax
    rep stosb

    mov edi, [multiboot2_info]

    call kernel_main

.hang:
    cli
    hlt
    jmp .hang

.code32
.no_cpuid:
    mov al, 'C'
    jmp .error

.no_long_mode:
    mov al, 'L'
    jmp .error

.error:
    mov dword ptr [0xb8000], 0x4f524f45
    mov dword ptr [0xb8004], 0x4f3a4f52
    mov dword ptr [0xb8008], 0x4f204f20
    movzx eax, al
    or eax, 0x0f00
    mov dword ptr [0xb800c], eax
    hlt
    jmp .error

    .section .data
KERNEL_VMA = 0xFFFFFFFF80000000

multiboot2_magic:
    .long 0
multiboot2_info:
    .long 0

    .align 16
gdt64:
    .quad 0
    .quad 0x00AF9A000000FFFF
    .quad 0x00AF92000000FFFF
gdt64_end:

gdt64_ptr:
    .word gdt64_end - gdt64 - 1
    .quad gdt64 - KERNEL_VMA

    .section .bss
    .align 4096
pml4:
    .space 4096
pdpt:
    .space 4096
pd:
    .space 4096

    .align 16
boot_stack_bottom:
    .space 65536
boot_stack_top:
    "#
);

extern "C" {
    static multiboot2_magic: u32;
    static multiboot2_info: u32;
}

pub unsafe fn get_boot_info() -> *const u8 {
    multiboot2_info as *const u8
}
