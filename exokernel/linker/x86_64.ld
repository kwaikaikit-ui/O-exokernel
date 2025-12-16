/* linker/x86_64.ld - x86_64架构链接脚本 */
OUTPUT_FORMAT("elf64-x86-64")
OUTPUT_ARCH(i386:x86-64)
ENTRY(_start)

KERNEL_VMA = 0xFFFFFFFF80000000;
KERNEL_LMA = 0x100000;

SECTIONS {
    . = KERNEL_LMA;

    /* Multiboot2头部必须在文件开头 */
    .multiboot2 : {
        KEEP(*(.multiboot2))
    }

    /* 切换到高地址空间 */
    . = KERNEL_VMA + ALIGN(., 4K);

    __kernel_start = .;

    .text : AT(ADDR(.text) - KERNEL_VMA) {
        __text_start = .;
        *(.text.boot)
        *(.text .text.*)
        __text_end = .;
    }

    .rodata ALIGN(4K) : AT(ADDR(.rodata) - KERNEL_VMA) {
        __rodata_start = .;
        *(.rodata .rodata.*)
        __rodata_end = .;
    }

    .data ALIGN(4K) : AT(ADDR(.data) - KERNEL_VMA) {
        __data_start = .;
        *(.data .data.*)
        __data_end = .;
    }

    .bss ALIGN(4K) : AT(ADDR(.bss) - KERNEL_VMA) {
        __bss_start = .;
        *(.bss .bss.*)
        *(COMMON)
        __bss_end = .;
    }

    __kernel_end = .;

    /* 丢弃不需要的段 */
    /DISCARD/ : {
        *(.comment)
        *(.eh_frame)
        *(.eh_frame_hdr)
        *(.note.*)
        *(.gcc_except_table)
    }
}

SECTIONS {
    . = 0x100000;  /* 1MB 处加载 */

    /* Multiboot2 头部必须在前 8KB 内 */
    .multiboot2 : {
        KEEP(*(.multiboot2))
    }

    /* 其余段... */
}
