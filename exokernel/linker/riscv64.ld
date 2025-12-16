/* linker/riscv64.ld - RISC-V 64位架构链接脚本 */
OUTPUT_ARCH(riscv)
ENTRY(_start)

/* RISC-V QEMU virt机器起始地址 */
KERNEL_BASE = 0x80000000;

SECTIONS {
    . = KERNEL_BASE;

    __kernel_start = .;

    /* 启动代码 */
    .text.boot : {
        __text_boot_start = .;
        KEEP(*(.text.boot))
        __text_boot_end = .;
    }

    /* 代码段 */
    .text ALIGN(4K) : {
        __text_start = .;
        *(.text .text.*)

        /* RISC-V特定的初始化代码 */
        . = ALIGN(8);
        __init_array_start = .;
        KEEP(*(.init_array))
        __init_array_end = .;

        __text_end = .;
    }

    /* 只读数据 */
    .rodata ALIGN(4K) : {
        __rodata_start = .;
        *(.rodata .rodata.*)
        *(.srodata .srodata.*)
        __rodata_end = .;
    }

    /* 数据段 */
    .data ALIGN(4K) : {
        __data_start = .;
        *(.data .data.*)
        *(.sdata .sdata.*)

        /* 全局指针 */
        . = ALIGN(8);
        __global_pointer$ = . + 0x800;
        *(.sdata2 .sdata2.*)

        __data_end = .;
    }

    /* BSS段 */
    .bss ALIGN(4K) : {
        __bss_start = .;
        *(.bss .bss.*)
        *(.sbss .sbss.*)
        *(COMMON)
        . = ALIGN(8);
        __bss_end = .;
    }

    /* 栈空间 */
    .stack ALIGN(16) (NOLOAD) : {
        __stack_bottom = .;
        . += 0x10000; /* 64KB */
        __stack_top = .;
    }

    __kernel_end = .;

    /* 设备树存储 */
    .dtb ALIGN(4K) (NOLOAD) : {
        __dtb_start = .;
        . += 0x100000; /* 1MB */
        __dtb_end = .;
    }

    /* 保留页表空间 */
    .page_tables ALIGN(4K) (NOLOAD) : {
        __page_tables_start = .;
        . += 0x100000; /* 1MB for page tables */
        __page_tables_end = .;
    }

    /DISCARD/ : {
        *(.comment)
        *(.eh_frame)
        *(.eh_frame_hdr)
        *(.note.*)
        *(.riscv.attributes)
    }
}
