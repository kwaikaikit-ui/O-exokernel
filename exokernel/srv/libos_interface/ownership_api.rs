//! 特性：
//! - 简洁的高层 API（隐藏 Capability 复杂性）
//! - 编译期 + 运行期双重安全保障
//! - 支持借用语义（readonly/exclusive/frozen）
//! - 跨进程共享与授权
//! - 引用计数共享内存
//! - 审计追踪与统计
//! - 与完整版 Capability 系统无缝互操作

use crate::capability::{
    ProcessId, ThreadId, ResourceId, ResourceType, CapabilityHandle,
    access, lifetime, ScopeKind, CapError,
    bind_resource_exclusive, bind_resource_readonly, bind_resource_scoped,
    borrow_shared_ro, borrow_exclusive, release_shared, release_exclusive,
    freeze_exclusive, unfreeze_exclusive,
    grant_readonly, grant_exclusive, transfer_resource,
    revoke_capability, revoke_capability_deferred,
    verify_capability_fast,
};
use alloc::sync::Arc;
use alloc::vec::Vec;
use core::marker::PhantomData;
use core::ops::{Deref, DerefMut};
use spin::Mutex;

// ========== 物理地址包装 ==========

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct PhysicalAddr(usize);

impl PhysicalAddr {
    pub const fn new(addr: usize) -> Self { Self(addr) }
    pub const fn as_usize(self) -> usize { self.0 }
    pub const fn as_u64(self) -> u64 { self.0 as u64 }
}

// ========== 类型 1：独占所有的物理页 ==========

/// 独占所有的物理页（Rust 所有权语义）
///
/// 特点：
/// - 编译期保证唯一所有者
/// - Drop 时自动撤销能力并释放页
/// - 可降级为只读借用
/// - 可转移给其他进程
pub struct OwnedPage {
    handle: CapabilityHandle<access::Exclusive, lifetime::Process>,
    addr: PhysicalAddr,
    owner_pid: u32,
}

impl OwnedPage {
    /// 分配新物理页
    pub fn alloc(pid: ProcessId) -> Result<Self, AllocError> {
        let addr = alloc_physical_page().ok_or(AllocError::OutOfMemory)?;
        let rid = ResourceId::from_page_addr(addr.as_usize());
        let handle = bind_resource_exclusive(pid, rid)
            .map_err(|e| AllocError::CapabilityError(e))?;

        Ok(Self {
            handle,
            addr,
            owner_pid: pid.as_u32(),
        })
    }

    /// 从已有地址创建（需要验证权限）
    pub fn from_addr(pid: ProcessId, addr: PhysicalAddr) -> Result<Self, AllocError> {
        let rid = ResourceId::from_page_addr(addr.as_usize());
        // 验证是否已拥有此地址的能力
        if !verify_capability_fast(pid, rid, crate::capability::caps::RW | crate::capability::caps::MAP) {
            return Err(AllocError::PermissionDenied);
        }
        let handle = bind_resource_exclusive(pid, rid)
            .map_err(|e| AllocError::CapabilityError(e))?;

        Ok(Self {
            handle,
            addr,
            owner_pid: pid.as_u32(),
        })
    }

    /// 获取物理地址
    pub fn addr(&self) -> PhysicalAddr {
        self.addr
    }

    /// 获取底层 Capability 句柄（高级用法）
    pub fn capability(&self) -> &CapabilityHandle<access::Exclusive, lifetime::Process> {
        &self.handle
    }

    /// 降级为只读借用（不释放所有权）
    pub fn as_readonly(&self, tid: ThreadId) -> Result<BorrowedPageRO<'_>, CapError> {
        let frozen = freeze_exclusive(&self.handle, tid)?;
        Ok(BorrowedPageRO {
            handle: frozen,
            addr: self.addr,
            tid,
            _phantom: PhantomData,
        })
    }

    /// 获取可写切片（unsafe：调用者需保证无别名）
    pub unsafe fn as_slice_mut(&mut self) -> &mut [u8] {
        core::slice::from_raw_parts_mut(
            self.addr.as_usize() as *mut u8,
            crate::arch::PAGE_SIZE
        )
    }

    /// 转移给其他进程（消耗 self）
    pub fn transfer_to(self, to_pid: ProcessId) -> Result<(), CapError> {
        let from_pid = ProcessId::new(self.owner_pid);
        let rid = ResourceId::from_page_addr(self.addr.as_usize());
        transfer_resource(from_pid, to_pid, rid)?;
        // self 会 drop，但已转移，避免二次释放
        core::mem::forget(self);
        Ok(())
    }

    /// 立即撤销并释放（不等待 Drop）
    pub fn revoke_now(self) -> Result<(), CapError> {
        revoke_capability(&self.handle)?;
        // 防止 Drop 二次释放
        core::mem::forget(self);
        Ok(())
    }

    /// 延迟撤销（当前有借用时不会立即释放）
    pub fn revoke_deferred(self) -> Result<(), CapError> {
        revoke_capability_deferred(&self.handle)?;
        core::mem::forget(self);
        Ok(())
    }
}

impl Drop for OwnedPage {
    fn drop(&mut self) {
        // 尝试撤销能力并释放物理页
        let _ = revoke_capability(&self.handle);
        free_physical_page(self.addr);
    }
}

// ========== 类型 2：只读借用的页 ==========

/// 只读借用的页（类似 &T）
///
/// 特点：
/// - 编译期保证只读
/// - 多个只读借用可共存
/// - Drop 时自动释放借用
pub struct BorrowedPageRO<'a> {
    handle: CapabilityHandle<access::FrozenShared>,
    addr: PhysicalAddr,
    tid: ThreadId,
    _phantom: PhantomData<&'a ()>,
}

impl<'a> BorrowedPageRO<'a> {
    /// 显式借用已有页
    pub fn borrow(
        page: &'a OwnedPage,
        tid: ThreadId,
        scope: ScopeKind,
    ) -> Result<Self, CapError> {
        let frozen = freeze_exclusive(&page.handle, tid)?;
        borrow_shared_ro(&page.handle, tid, scope)?;
        Ok(Self {
            handle: frozen,
            addr: page.addr,
            tid,
            _phantom: PhantomData,
        })
    }

    /// 从共享页借用
    pub fn borrow_shared(
        page: &'a SharedPage,
        tid: ThreadId,
        scope: ScopeKind,
    ) -> Result<Self, CapError> {
        let inner = page.inner.lock();
        let handle = CapabilityHandle::new(
            inner.handle.as_raw().0,
            inner.handle.as_raw().1,
            ScopeKind::Process,
            0, // 简化：从 RO_DATA 读取
        );
        borrow_shared_ro(&handle, tid, scope)?;
        Ok(Self {
            handle,
            addr: inner.addr,
            tid,
            _phantom: PhantomData,
        })
    }

    pub fn addr(&self) -> PhysicalAddr {
        self.addr
    }

    /// 获取只读切片
    pub fn as_slice(&self) -> &[u8] {
        unsafe {
            core::slice::from_raw_parts(
                self.addr.as_usize() as *const u8,
                crate::arch::PAGE_SIZE
            )
        }
    }
}

impl<'a> Drop for BorrowedPageRO<'a> {
    fn drop(&mut self) {
        // 使用 FrozenShared 的释放 API
        let _ = crate::capability::release_shared_frozen(&self.handle, self.tid);
    }
}

// ========== 类型 3：独占借用的页 ==========

/// 独占借用的页（类似 &mut T）
///
/// 特点：
/// - 编译期保证独占访问
/// - 运行期检查借用冲突
/// - Drop 时自动释放借用
pub struct BorrowedPageRW<'a> {
    handle: CapabilityHandle<access::Exclusive>,
    addr: PhysicalAddr,
    tid: ThreadId,
    _phantom: PhantomData<&'a mut ()>,
}

impl<'a> BorrowedPageRW<'a> {
    /// 独占借用页
    pub fn borrow_mut(
        page: &'a mut OwnedPage,
        tid: ThreadId,
        scope: ScopeKind,
    ) -> Result<Self, CapError> {
        borrow_exclusive(&page.handle, tid, scope)?;
        // 这里不能移动 handle，需要用原始索引重建
        let (idx, gen) = page.handle.as_raw();
        let handle = CapabilityHandle::new(idx, gen, scope, 0);
        Ok(Self {
            handle,
            addr: page.addr,
            tid,
            _phantom: PhantomData,
        })
    }

    pub fn addr(&self) -> PhysicalAddr {
        self.addr
    }

    pub fn as_slice_mut(&mut self) -> &mut [u8] {
        unsafe {
            core::slice::from_raw_parts_mut(
                self.addr.as_usize() as *mut u8,
                crate::arch::PAGE_SIZE
            )
        }
    }
}

impl<'a> Drop for BorrowedPageRW<'a> {
    fn drop(&mut self) {
        let _ = release_exclusive(&self.handle, self.tid);
    }
}

// ========== 类型 4：引用计数共享页 ==========

/// 引用计数共享页（类似 Arc）
///
/// 特点：
/// - 多个所有者共享
/// - 最后一个所有者释放时回收
/// - 线程安全
/// - 支持跨进程共享
pub struct SharedPage {
    inner: Arc<Mutex<SharedPageInner>>,
}

struct SharedPageInner {
    handle: CapabilityHandle<access::ReadOnly, lifetime::Process>,
    addr: PhysicalAddr,
    owner_pid: u32,
}

impl SharedPage {
    /// 从独占页创建共享页
    pub fn from_owned(page: OwnedPage) -> Self {
        let handle = page.handle.downgrade();
        let addr = page.addr;
        let owner_pid = page.owner_pid;
        core::mem::forget(page); // 避免 drop

        Self {
            inner: Arc::new(Mutex::new(SharedPageInner {
                handle,
                addr,
                owner_pid,
            })),
        }
    }

    /// 授权只读访问给其他进程
    pub fn grant_readonly(&self, grantee_pid: ProcessId) -> Result<Self, CapError> {
        let inner = self.inner.lock();
        let grantor_pid = ProcessId::new(inner.owner_pid);
        let rid = ResourceId::from_page_addr(inner.addr.as_usize());
        let new_handle = grant_readonly(grantor_pid, grantee_pid, rid)?;

        Ok(Self {
            inner: Arc::new(Mutex::new(SharedPageInner {
                handle: new_handle,
                addr: inner.addr,
                owner_pid: grantee_pid.as_u32(),
            })),
        })
    }

    pub fn addr(&self) -> PhysicalAddr {
        self.inner.lock().addr
    }

    pub fn as_slice(&self) -> SharedSlice {
        SharedSlice {
            page: Arc::clone(&self.inner),
        }
    }

    /// 克隆引用（增加引用计数）
    pub fn share(&self) -> Self {
        Self {
            inner: Arc::clone(&self.inner),
        }
    }

    /// 获取引用计数
    pub fn ref_count(&self) -> usize {
        Arc::strong_count(&self.inner)
    }
}

impl Clone for SharedPage {
    fn clone(&self) -> Self {
        self.share()
    }
}

impl Drop for SharedPage {
    fn drop(&mut self) {
        if Arc::strong_count(&self.inner) == 1 {
            // 最后一个引用，撤销能力并释放
            let inner = self.inner.lock();
            let _ = revoke_capability(&inner.handle);
            free_physical_page(inner.addr);
        }
    }
}

/// 共享页的切片包装
pub struct SharedSlice {
    page: Arc<Mutex<SharedPageInner>>,
}

impl Deref for SharedSlice {
    type Target = [u8];
    fn deref(&self) -> &Self::Target {
        let inner = self.page.lock();
        unsafe {
            core::slice::from_raw_parts(
                inner.addr.as_usize() as *const u8,
                crate::arch::PAGE_SIZE
            )
        }
    }
}

// ========== 类型 5：页向量（批量管理） ==========

/// 页向量（自动管理多个页）
pub struct PageVec {
    pages: Vec<OwnedPage>,
    owner_pid: u32,
}

impl PageVec {
    pub fn new(owner_pid: u32) -> Self {
        Self {
            pages: Vec::new(),
            owner_pid,
        }
    }

    pub fn with_capacity(owner_pid: u32, capacity: usize) -> Self {
        Self {
            pages: Vec::with_capacity(capacity),
            owner_pid,
        }
    }

    pub fn push(&mut self, page: OwnedPage) {
        debug_assert_eq!(page.owner_pid, self.owner_pid);
        self.pages.push(page);
    }

    pub fn pop(&mut self) -> Option<OwnedPage> {
        self.pages.pop()
    }

    pub fn len(&self) -> usize {
        self.pages.len()
    }

    pub fn is_empty(&self) -> bool {
        self.pages.is_empty()
    }

    pub fn get(&self, index: usize) -> Option<&OwnedPage> {
        self.pages.get(index)
    }

    pub fn get_mut(&mut self, index: usize) -> Option<&mut OwnedPage> {
        self.pages.get_mut(index)
    }

    pub fn iter(&self) -> impl Iterator<Item = &OwnedPage> {
        self.pages.iter()
    }

    /// 批量撤销所有页（延迟模式）
    pub fn revoke_all_deferred(mut self) -> Result<(), CapError> {
        for page in self.pages.drain(..) {
            page.revoke_deferred()?;
        }
        Ok(())
    }
}

// ========== 系统调用接口 ==========

pub struct Syscall;

impl Syscall {
    /// 分配单个物理页
    pub fn alloc_page(pid: ProcessId) -> Result<OwnedPage, AllocError> {
        OwnedPage::alloc(pid)
    }

    /// 批量分配物理页
    pub fn alloc_pages(pid: ProcessId, count: usize) -> Result<PageVec, AllocError> {
        let mut vec = PageVec::with_capacity(pid.as_u32(), count);
        for _ in 0..count {
            match OwnedPage::alloc(pid) {
                Ok(page) => vec.push(page),
                Err(e) => {
                    if vec.is_empty() {
                        return Err(e);
                    } else {
                        break; // 部分成功
                    }
                }
            }
        }
        Ok(vec)
    }

    /// 分配共享页
    pub fn alloc_shared_page(pid: ProcessId) -> Result<SharedPage, AllocError> {
        let page = OwnedPage::alloc(pid)?;
        Ok(SharedPage::from_owned(page))
    }

    /// 从地址创建页（需验证权限）
    pub fn page_from_addr(pid: ProcessId, addr: PhysicalAddr) -> Result<OwnedPage, AllocError> {
        OwnedPage::from_addr(pid, addr)
    }

    /// 授权页给其他进程（只读）
    pub fn grant_page_readonly(
        grantor_pid: ProcessId,
        grantee_pid: ProcessId,
        addr: PhysicalAddr,
    ) -> Result<OwnedPage, AllocError> {
        let rid = ResourceId::from_page_addr(addr.as_usize());
        let handle = grant_readonly(grantor_pid, grantee_pid, rid)
            .map_err(|e| AllocError::CapabilityError(e))?;

        Ok(OwnedPage {
            handle,
            addr,
            owner_pid: grantee_pid.as_u32(),
        })
    }

    /// 授权页给其他进程（独占）
    pub fn grant_page_exclusive(
        grantor_pid: ProcessId,
        grantee_pid: ProcessId,
        addr: PhysicalAddr,
    ) -> Result<OwnedPage, AllocError> {
        let rid = ResourceId::from_page_addr(addr.as_usize());
        let handle = grant_exclusive(grantor_pid, grantee_pid, rid)
            .map_err(|e| AllocError::CapabilityError(e))?;

        Ok(OwnedPage {
            handle,
            addr,
            owner_pid: grantee_pid.as_u32(),
        })
    }

    /// 转移页所有权
    pub fn transfer_page(
        page: OwnedPage,
        to_pid: ProcessId,
    ) -> Result<(), AllocError> {
        page.transfer_to(to_pid).map_err(|e| AllocError::CapabilityError(e))
    }

    /// 系统信息
    pub fn system_info() -> SystemInfo {
        let stats = crate::capability::get_stats();
        SystemInfo {
            free_pages: unsafe { crate::mm::physical::free_pages() },
            page_size: crate::arch::PAGE_SIZE,
            capability_stats: stats,
        }
    }
}

// ========== 错误类型 ==========

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AllocError {
    OutOfMemory,
    PermissionDenied,
    CapabilityError(CapError),
}

impl From<CapError> for AllocError {
    fn from(e: CapError) -> Self {
        AllocError::CapabilityError(e)
    }
}

// ========== 系统信息 ==========

#[derive(Debug, Clone)]
pub struct SystemInfo {
    pub free_pages: usize,
    pub page_size: usize,
    pub capability_stats: crate::capability::CapabilityStats,
}

// ========== 底层物理内存函数（需实现） ==========

fn alloc_physical_page() -> Option<PhysicalAddr> {
    // 调用物理内存分配器
    unsafe { crate::mm::physical::alloc_page().map(PhysicalAddr::new) }
}

fn free_physical_page(addr: PhysicalAddr) {
    // 调用物理内存分配器释放
    unsafe { crate::mm::physical::free_page(addr.as_usize()) }
}

// ========== 使用示例 ==========

#[cfg(test)]
mod examples {
    use super::*;

    fn example_basic() -> Result<(), AllocError> {
        let pid = ProcessId::new(1);

        // 1. 单页分配
        let page = Syscall::alloc_page(pid)?;
        println!("Allocated page at {:?}", page.addr());
        // page 离开作用域时自动释放

        // 2. 批量分配
        let pages = Syscall::alloc_pages(pid, 10)?;
        println!("Allocated {} pages", pages.len());
        // pages 离开作用域时自动释放所有页

        Ok(())
    }

    fn example_borrowing() -> Result<(), AllocError> {
        let pid = ProcessId::new(1);
        let tid = ThreadId::new(1);
        let mut page = Syscall::alloc_page(pid)?;

        // 只读借用
        {
            let borrowed = page.as_readonly(tid)?;
            let data = borrowed.as_slice();
            println!("Read {} bytes", data.len());
        } // borrowed 自动释放

        // 独占借用
        {
            let mut borrowed = BorrowedPageRW::borrow_mut(&mut page, tid, ScopeKind::Thread(tid))?;
            let data = borrowed.as_slice_mut();
            data[0] = 42;
        } // borrowed 自动释放

        Ok(())
    }

    fn example_sharing() -> Result<(), AllocError> {
        let pid1 = ProcessId::new(1);
        let pid2 = ProcessId::new(2);

        // 创建共享页
        let shared = Syscall::alloc_shared_page(pid1)?;

        // 克隆引用
        let shared2 = shared.share();
        println!("Ref count: {}", shared.ref_count()); // 2

        // 授权给其他进程
        let shared_for_pid2 = shared.grant_readonly(pid2)?;

        // 所有引用释放时自动回收
        Ok(())
    }

    fn example_transfer() -> Result<(), AllocError> {
        let pid1 = ProcessId::new(1);
        let pid2 = ProcessId::new(2);

        let page = Syscall::alloc_page(pid1)?;

        // 转移所有权给 pid2
        Syscall::transfer_page(page, pid2)?;
        // page 已被消费，pid1 无法再访问

        Ok(())
    }
}

