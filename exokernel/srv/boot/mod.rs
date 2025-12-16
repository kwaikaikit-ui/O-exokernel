// src/boot/mod.rs
//! 启动信息解析 - 支持Multiboot2和设备树

pub mod multiboot2;
pub mod devicetree;

use alloc::vec::Vec;

#[derive(Debug, Clone, Copy)]
pub struct MemoryRegion {
    pub base: usize,
    pub size: usize,
    pub available: bool,
}

pub fn parse_boot_info(boot_info: *const u8) -> Vec<MemoryRegion> {
    if boot_info.is_null() {
        return Vec::new();
    }

    #[cfg(target_arch = "x86_64")]
    {
        multiboot2::parse(boot_info)
    }

    #[cfg(any(target_arch = "aarch64", target_arch = "riscv64"))]
    {
        devicetree::parse(boot_info)
    }

    #[cfg(target_arch = "loongarch64")]
    {
        // LoongArch通常使用UEFI或自定义格式
        Vec::new()
    }
}
