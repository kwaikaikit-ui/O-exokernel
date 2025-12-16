/* linker/aarch64.ld - AArch64架构链接脚本 */
OUTPUT_FORMAT("elf64-littleaarch64")
OUTPUT_ARCH(aarch64)
ENTRY(_start)

/* AArch64设备常用起始地址 */
KERNEL_BASE = 0x40000000;

SECTIONS {
    . = KERNEL_BASE;

    __kernel_start = .;

    /* 启动代码必须在最前面 */
    .text.boot : {
        __text_boot_start = .;
        *(.text.boot)
        __text_boot_end = .;
    }

    /* 代码段 */
    .text ALIGN(4K) : {
        __text_start = .;
        *(.text .text.*)
        __text_end = .;
    }

    /* 只读数据段 */
    .rodata ALIGN(4K) : {
        __rodata_start = .;
        *(.rodata .rodata.*)
        __rodata_end = .;
    }

    /* 已初始化数据段 */
    .data ALIGN(4K) : {
        __data_start = .;
        *(.data .data.*)

        /* 全局偏移表 */
        . = ALIGN(8);
        __got_start = .;
        *(.got .got.*)
        __got_end = .;

        __data_end = .;
    }

    /* 未初始化数据段 */
    .bss ALIGN(4K) : {
        __bss_start = .;
        *(.bss .bss.*)
        *(COMMON)
        . = ALIGN(16);
        __bss_end = .;
    }

    /* 栈空间（可选） */
    .stack ALIGN(4K) (NOLOAD) : {
        __stack_bottom = .;
        . += 0x10000; /* 64KB */
        __stack_top = .;
    }

    __kernel_end = .;

    /* 设备树blob存储区域 */
    .dtb ALIGN(4K) (NOLOAD) : {
        __dtb_start = .;
        . += 0x100000; /* 1MB for DTB */
        __dtb_end = .;
    }

    /DISCARD/ : {
        *(.comment)
        *(.eh_frame)
        *(.eh_frame_hdr)
        *(.note.*)
        *(.ARM.exidx*)
        *(.ARM.extab*)
    }
}
