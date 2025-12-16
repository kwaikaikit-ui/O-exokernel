// src/arch/mod.rs
//! 多架构抽象层

#[cfg(target_arch = "x86_64")]
#[path = "x86_64/mod.rs"]
pub mod imp;

#[cfg(target_arch = "aarch64")]
#[path = "aarch64/mod.rs"]
pub mod imp;

#[cfg(target_arch = "riscv64")]
#[path = "riscv64/mod.rs"]
pub mod imp;

#[cfg(target_arch = "loongarch64")]
#[path = "loongarch64/mod.rs"]
pub mod imp;

// 重新导出当前架构的实现
pub use imp::*;

/// 页面大小
pub const PAGE_SIZE: usize = 4096;
pub const PAGE_SHIFT: usize = 12;

/// 架构名称
#[cfg(target_arch = "x86_64")]
pub const ARCH_NAME: &str = "x86_64";

#[cfg(target_arch = "aarch64")]
pub const ARCH_NAME: &str = "aarch64";

#[cfg(target_arch = "riscv64")]
pub const ARCH_NAME: &str = "riscv64";

#[cfg(target_arch = "loongarch64")]
pub const ARCH_NAME: &str = "loongarch64";

/// 架构通用trait
pub trait Architecture {
    fn early_init();
    fn halt();
    fn enable_interrupts();
    fn disable_interrupts();
    fn write_serial(byte: u8);
}
