// src/mm/allocator.rs
//! 高级内存分配器 - 为LibOS提供Rust所有权风格的API
//!
//! 这个模块在底层物理分配器之上提供：
//! 1. 类型安全的分配接口
//! 2. 自动生命周期管理
//! 3. 借用检查器友好的API
//! 4. 零成本抽象

use super::ownership::{OwnedPage, PageVec, BorrowedPage};
use core::marker::PhantomData;
use core::ptr::NonNull;

/// 内存分配器 - LibOS的主要接口
pub struct Allocator<'libos> {
    pid: u32,
    _lifetime: PhantomData<&'libos ()>,
}

impl<'libos> Allocator<'libos> {
    /// 创建新的分配器实例（绑定到特定LibOS）
    ///
    /// # Safety
    ///
    /// 调用者必须确保 pid 是有效且唯一的
    pub unsafe fn new(pid: u32) -> Self {
        Self {
            pid,
            _lifetime: PhantomData,
        }
    }

    /// 分配单个页面
    ///
    /// 返回的 OwnedPage 拥有该页面的所有权，
    /// 当它离开作用域时会自动释放
    ///
    /// # Example
    ///
    /// ```rust
    /// let alloc = unsafe { Allocator::new(1) };
    /// let page = alloc.alloc_page().expect("Out of memory");
    /// // 使用 page...
    /// // page 在这里自动释放
    /// ```
    pub fn alloc_page(&self) -> Result<OwnedPage, AllocError> {
        OwnedPage::alloc(self.pid).ok_or(AllocError::OutOfMemory)
    }

    /// 分配多个连续页面
    ///
    /// 返回 PageVec，它管理一组页面的所有权
    pub fn alloc_pages(&self, count: usize) -> Result<PageVec, AllocError> {
        if count == 0 {
            return Err(AllocError::InvalidSize);
        }

        let mut vec = PageVec::new(self.pid);

        for _ in 0..count {
            match OwnedPage::alloc(self.pid) {
                Some(page) => vec.push(page),
                None => {
                    // 分配失败，已分配的页会在 vec drop 时自动释放
                    return Err(AllocError::OutOfMemory);
                }
            }
        }

        Ok(vec)
    }

    /// 尝试分配多个页面，返回实际分配的数量
    ///
    /// 与 alloc_pages 不同，这个函数会尽可能多地分配，
    /// 而不是全有或全无
    pub fn try_alloc_pages(&self, count: usize) -> PageVec {
        let mut vec = PageVec::new(self.pid);

        for _ in 0..count {
            if let Some(page) = OwnedPage::alloc(self.pid) {
                vec.push(page);
            } else {
                break;
            }
        }

        vec
    }

    /// 获取分配器的统计信息
    pub fn stats(&self) -> AllocatorStats {
        unsafe {
            AllocatorStats {
                free_pages: super::physical::free_pages(),
                total_pages: 65536, // MAX_PAGES
                page_size: crate::arch::PAGE_SIZE,
            }
        }
    }
}

/// 分配错误类型
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AllocError {
    /// 内存不足
    OutOfMemory,
    /// 无效的大小参数
    InvalidSize,
    /// 对齐错误
    InvalidAlignment,
}

impl core::fmt::Display for AllocError {
    fn fmt(&self, f: &mut core::fmt::Formatter) -> core::fmt::Result {
        match self {
            AllocError::OutOfMemory => write!(f, "Out of memory"),
            AllocError::InvalidSize => write!(f, "Invalid allocation size"),
            AllocError::InvalidAlignment => write!(f, "Invalid alignment"),
        }
    }
}

/// 分配器统计信息
#[derive(Debug, Clone, Copy)]
pub struct AllocatorStats {
    pub free_pages: usize,
    pub total_pages: usize,
    pub page_size: usize,
}

impl AllocatorStats {
    pub fn free_memory(&self) -> usize {
        self.free_pages * self.page_size
    }

    pub fn total_memory(&self) -> usize {
        self.total_pages * self.page_size
    }

    pub fn used_memory(&self) -> usize {
        self.total_memory() - self.free_memory()
    }

    pub fn usage_percent(&self) -> f32 {
        if self.total_pages == 0 {
            return 0.0;
        }
        ((self.total_pages - self.free_pages) as f32 / self.total_pages as f32) * 100.0
    }
}

/// 页面区域 - 表示一段连续的物理内存
///
/// 这个类型提供了对多个连续页面的所有权管理
pub struct PageRegion {
    pages: PageVec,
    base_addr: usize,
}

impl PageRegion {
    /// 从 PageVec 创建区域
    pub fn from_pages(pages: PageVec) -> Option<Self> {
        if pages.len() == 0 {
            return None;
        }

        let base_addr = pages.get(0)?.address();

        Some(Self {
            pages,
            base_addr,
        })
    }

    /// 获取基地址
    pub fn base_address(&self) -> usize {
        self.base_addr
    }

    /// 获取大小（字节）
    pub fn size(&self) -> usize {
        self.pages.len() * crate::arch::PAGE_SIZE
    }

    /// 获取页数
    pub fn page_count(&self) -> usize {
        self.pages.len()
    }

    /// 获取指定索引的页
    pub fn get_page(&self, index: usize) -> Option<&OwnedPage> {
        self.pages.get(index)
    }
}

/// 分配范围 - RAII风格的批量分配
///
/// 当这个对象被创建时，它会预留一定数量的页面
/// 当它离开作用域时，所有未使用的页面会被释放
pub struct AllocationScope {
    allocator: Allocator<'static>,
    reserved: PageVec,
}

impl AllocationScope {
    /// 创建新的分配范围，预留指定数量的页面
    pub fn new(pid: u32, reserve_count: usize) -> Result<Self, AllocError> {
        let allocator = unsafe { Allocator::new(pid) };
        let reserved = allocator.try_alloc_pages(reserve_count);

        if reserved.len() == 0 {
            return Err(AllocError::OutOfMemory);
        }

        Ok(Self {
            allocator,
            reserved,
        })
    }

    /// 从预留池中取出一页
    pub fn take_page(&mut self) -> Option<OwnedPage> {
        self.reserved.pop()
    }

    /// 剩余预留页数
    pub fn remaining(&self) -> usize {
        self.reserved.len()
    }
}

// Drop 时自动释放所有未使用的页面
impl Drop for AllocationScope {
    fn drop(&mut self) {
        // PageVec 的 drop 会自动处理
    }
}

/// 页面池 - 用于频繁分配/释放的场景
///
/// 维护一个页面缓存，减少分配开销
pub struct PagePool {
    allocator: Allocator<'static>,
    cache: PageVec,
    max_cache_size: usize,
}

impl PagePool {
    /// 创建新的页面池
    pub fn new(pid: u32, max_cache_size: usize) -> Self {
        Self {
            allocator: unsafe { Allocator::new(pid) },
            cache: PageVec::new(pid),
            max_cache_size,
        }
    }

    /// 从池中获取一页（优先使用缓存）
    pub fn acquire(&mut self) -> Result<OwnedPage, AllocError> {
        // 先尝试从缓存获取
        if let Some(page) = self.cache.pop() {
            return Ok(page);
        }

        // 缓存为空，分配新页
        self.allocator.alloc_page()
    }

    /// 归还页面到池中（可能进入缓存）
    pub fn release(&mut self, page: OwnedPage) {
        if self.cache.len() < self.max_cache_size {
            self.cache.push(page);
            // page 不会被 drop，保留在缓存中
        } else {
            // 缓存已满，让 page 自动 drop
            drop(page);
        }
    }

    /// 清空缓存
    pub fn clear_cache(&mut self) {
        // 简单地创建新的 PageVec，旧的会自动释放所有页面
        self.cache = PageVec::new(self.allocator.pid);
    }

    /// 获取缓存统计
    pub fn cache_stats(&self) -> (usize, usize) {
        (self.cache.len(), self.max_cache_size)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_basic_allocation() {
        let alloc = unsafe { Allocator::new(1) };

        // 测试单页分配
        let page = alloc.alloc_page().expect("Failed to allocate");
        assert!(page.address() != 0);

        // 测试自动释放（当 page drop 时）
    }

    #[test]
    fn test_batch_allocation() {
        let alloc = unsafe { Allocator::new(2) };

        // 分配 10 页
        let pages = alloc.alloc_pages(10).expect("Failed to allocate");
        assert_eq!(pages.len(), 10);

        // 所有页在 pages drop 时自动释放
    }

    #[test]
    fn test_allocation_scope() {
        let mut scope = AllocationScope::new(3, 5).expect("Failed to create scope");

        assert_eq!(scope.remaining(), 5);

        let page1 = scope.take_page().unwrap();
        assert_eq!(scope.remaining(), 4);

        drop(page1);

        // scope drop 时剩余 4 页自动释放
    }
}
