// src/arch/x86_64/multiboot2.rs
//! Multiboot2 协议头部和结构定义

use core::mem;

/// Multiboot2 魔数
pub const MULTIBOOT2_MAGIC: u32 = 0xE85250D6;

/// 架构标识
pub const MULTIBOOT2_ARCH_I386: u32 = 0;

/// 标签类型
pub mod tag_types {
    pub const END: u16 = 0;
    pub const INFORMATION_REQUEST: u16 = 1;
    pub const ADDRESS: u16 = 2;
    pub const ENTRY_ADDRESS: u16 = 3;
    pub const FLAGS: u16 = 4;
    pub const FRAMEBUFFER: u16 = 5;
    pub const MODULE_ALIGN: u16 = 6;
    pub const EFI_BOOT_SERVICES: u16 = 7;
    pub const EFI_I386_ENTRY: u16 = 8;
    pub const EFI_AMD64_ENTRY: u16 = 9;
    pub const RELOCATABLE: u16 = 10;
}

/// 信息请求类型
pub mod info_types {
    pub const BASIC_MEMINFO: u32 = 4;
    pub const BOOTDEV: u32 = 5;
    pub const MMAP: u32 = 6;
    pub const VBE_INFO: u32 = 7;
    pub const FRAMEBUFFER_INFO: u32 = 8;
    pub const ELF_SECTIONS: u32 = 9;
    pub const APM_TABLE: u32 = 10;
    pub const EFI_32_SYSTEM_TABLE: u32 = 11;
    pub const EFI_64_SYSTEM_TABLE: u32 = 12;
    pub const SMBIOS_TABLES: u32 = 13;
    pub const ACPI_OLD: u32 = 14;
    pub const ACPI_NEW: u32 = 15;
    pub const NETWORK_INFO: u32 = 16;
    pub const EFI_MMAP: u32 = 17;
    pub const EFI_BOOT_SERVICES_NOT_TERMINATED: u32 = 18;
    pub const EFI_32_IMAGE_HANDLE: u32 = 19;
    pub const EFI_64_IMAGE_HANDLE: u32 = 20;
    pub const IMAGE_LOAD_BASE_ADDR: u32 = 21;
}

/// Multiboot2 头部结构（在汇编中定义）
#[repr(C, align(8))]
pub struct Multiboot2Header {
    magic: u32,
    architecture: u32,
    header_length: u32,
    checksum: u32,
}

impl Multiboot2Header {
    pub const fn new(header_length: u32) -> Self {
        Self {
            magic: MULTIBOOT2_MAGIC,
            architecture: MULTIBOOT2_ARCH_I386,
            header_length,
            checksum: 0u32.wrapping_sub(MULTIBOOT2_MAGIC + MULTIBOOT2_ARCH_I386 + header_length),
        }
    }
}

/// 标签头部
#[repr(C, align(8))]
pub struct TagHeader {
    typ: u16,
    flags: u16,
    size: u32,
}

/// 信息请求标签
#[repr(C, align(8))]
pub struct InformationRequestTag {
    header: TagHeader,
    requests: [u32; 10], // 可以根据需要调整
}

/// 帧缓冲标签
#[repr(C, align(8))]
pub struct FramebufferTag {
    header: TagHeader,
    width: u32,
    height: u32,
    depth: u32,
}

/// 模块对齐标签
#[repr(C, align(8))]
pub struct ModuleAlignTag {
    header: TagHeader,
}

/// 可重定位标签
#[repr(C, align(8))]
pub struct RelocatableTag {
    header: TagHeader,
    min_addr: u32,
    max_addr: u32,
    align: u32,
    preference: u32,
}

/// 结束标签
#[repr(C, align(8))]
pub struct EndTag {
    typ: u16,
    flags: u16,
    size: u32,
}
