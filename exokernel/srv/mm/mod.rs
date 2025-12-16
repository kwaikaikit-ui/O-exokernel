// src/mm/mod.rs
//! 内存管理

pub mod physical;
pub mod ownership;
pub mod allocator;

// 重新导出常用类型
pub use allocator::{Allocator, AllocError, AllocatorStats, PagePool, AllocationScope};
pub use ownership::{OwnedPage, PageVec, BorrowedPage};

use alloc::vec::Vec;
use crate::boot::MemoryRegion;

pub fn init(regions: Vec<MemoryRegion>) {
    // 找到最大的可用内存区域
    let mut best_region: Option<&MemoryRegion> = None;

    for region in &regions {
        if region.available {
            if let Some(best) = best_region {
                if region.size > best.size {
                    best_region = Some(region);
                }
            } else {
                best_region = Some(region);
            }
        }
    }

    if let Some(region) = best_region {
        unsafe {
            physical::init(region.base, region.size);
        }
        crate::println!("  [MM] Using region: 0x{:x} + {}MB",
                        region.base, region.size / (1024 * 1024));
    }
}
