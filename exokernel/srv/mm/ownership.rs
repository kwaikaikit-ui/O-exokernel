// src/mm/ownership.rs
//! Rust所有权模型的物理页管理

use core::ptr::NonNull;
use core::marker::PhantomData;

/// 物理页 - 拥有所有权
pub struct OwnedPage {
    addr: usize,
    pid: u32,
    _marker: PhantomData<*mut u8>, // 不是Send/Sync
}

impl OwnedPage {
    /// 分配新页（获取所有权）
    pub fn alloc(pid: u32) -> Option<Self> {
        unsafe {
            super::physical::alloc_raw(pid).map(|addr| Self {
                addr,
                pid,
                _marker: PhantomData,
            })
        }
    }

    /// 获取物理地址（不可变借用）
    pub fn address(&self) -> usize {
        self.addr
    }

    /// 获取进程ID
    pub fn owner(&self) -> u32 {
        self.pid
    }

    /// 转移所有权到另一个进程
    pub fn transfer_to(mut self, new_pid: u32) -> Self {
        unsafe {
            super::physical::change_owner(self.addr, self.pid, new_pid)
                .expect("Transfer failed");
        }
        self.pid = new_pid;
        self
    }

    /// 创建共享引用（借用检查）
    pub fn share(&self) -> BorrowedPage {
        BorrowedPage {
            addr: self.addr,
            _lifetime: PhantomData,
        }
    }
}

impl Drop for OwnedPage {
    fn drop(&mut self) {
        unsafe {
            let _ = super::physical::free_raw(self.pid, self.addr);
        }
    }
}

/// 借用的页引用
pub struct BorrowedPage<'a> {
    addr: usize,
    _lifetime: PhantomData<&'a ()>,
}

impl<'a> BorrowedPage<'a> {
    pub fn address(&self) -> usize {
        self.addr
    }
}

/// 页集合 - Vec语义
pub struct PageVec {
    pages: alloc::vec::Vec<OwnedPage>,
    pid: u32,
}

impl PageVec {
    pub fn new(pid: u32) -> Self {
        Self {
            pages: alloc::vec::Vec::new(),
            pid,
        }
    }

    pub fn push(&mut self, page: OwnedPage) {
        assert_eq!(page.owner(), self.pid, "PID mismatch");
        self.pages.push(page);
    }

    pub fn pop(&mut self) -> Option<OwnedPage> {
        self.pages.pop()
    }

    pub fn len(&self) -> usize {
        self.pages.len()
    }

    pub fn get(&self, index: usize) -> Option<&OwnedPage> {
        self.pages.get(index)
    }
}

// 自动释放所有页
impl Drop for PageVec {
    fn drop(&mut self) {
        // Vec的drop会自动drop所有元素
    }
}
