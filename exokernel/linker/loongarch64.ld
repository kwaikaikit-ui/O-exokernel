/* linker/loongarch64.ld - LoongArch 64位架构链接脚本 */
OUTPUT_ARCH(loongarch64)
ENTRY(_start)

/* LoongArch通常起始地址 */
KERNEL_BASE = 0x9000000000000000;
KERNEL_LMA = 0x0000000001000000;

SECTIONS {
    . = KERNEL_BASE;

    __kernel_vma_start = .;
    __kernel_lma_start = KERNEL_LMA;

    /* 启动代码段 */
    .text.boot : AT(KERNEL_LMA) {
        __text_boot_start = .;
        KEEP(*(.text.boot))
        __text_boot_end = .;
    }

    /* 主代码段 */
    .text ALIGN(4K) : AT(ALIGN(KERNEL_LMA + SIZEOF(.text.boot), 4K)) {
        __text_start = .;
        *(.text .text.*)

        /* 中断向量表 */
        . = ALIGN(16);
        __exception_vectors_start = .;
        KEEP(*(.exception_vectors))
        __exception_vectors_end = .;

        __text_end = .;
    }

    /* 只读数据 */
    .rodata ALIGN(4K) : AT(ALIGN(LOADADDR(.text) + SIZEOF(.text), 4K)) {
        __rodata_start = .;
        *(.rodata .rodata.*)

        /* LoongArch特定的重定位数据 */
        . = ALIGN(8);
        __la_abs_start = .;
        *(.la_abs)
        __la_abs_end = .;

        __rodata_end = .;
    }

    /* 数据段 */
    .data ALIGN(4K) : AT(ALIGN(LOADADDR(.rodata) + SIZEOF(.rodata), 4K)) {
        __data_start = .;
        *(.data .data.*)

        /* 全局偏移表 */
        . = ALIGN(8);
        __got_start = .;
        *(.got .got.*)
        __got_end = .;

        __data_end = .;
    }

    /* BSS段 */
    .bss ALIGN(4K) : AT(ALIGN(LOADADDR(.data) + SIZEOF(.data), 4K)) {
        __bss_start = .;
        *(.bss .bss.*)
        *(COMMON)
        . = ALIGN(16);
        __bss_end = .;
    }

    /* 栈空间 */
    .stack ALIGN(16) (NOLOAD) : {
        __stack_bottom = .;
        . += 0x20000; /* 128KB栈 */
        __stack_top = .;
    }

    /* 页表空间 */
    .page_tables ALIGN(4K) (NOLOAD) : {
        __page_tables_start = .;
        . += 0x200000; /* 2MB页表 */
        __page_tables_end = .;
    }

    /* TLB重填处理代码（LoongArch特有） */
    .tlb_refill ALIGN(4K) : {
        __tlb_refill_start = .;
        KEEP(*(.tlb_refill))
        __tlb_refill_end = .;
    }

    __kernel_vma_end = .;
    __kernel_size = __kernel_vma_end - __kernel_vma_start;

    /DISCARD/ : {
        *(.comment)
        *(.eh_frame)
        *(.eh_frame_hdr)
        *(.note.*)
        *(.interp)
        *(.dynamic)
    }
}
