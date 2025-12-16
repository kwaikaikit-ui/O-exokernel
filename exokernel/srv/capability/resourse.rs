//! - 真源：RO_DATA（表项），WR_DATA 仅存索引/队列；锁顺序 WR_DATA -> RO_DATA.write
//! - generation 仅在 free/revoke 时递增；分配时读取当前值（seL4 模型）
//! - Per-CPU 缓存：命中需校验；free/reuse 时失效
//! - quick_cache/resource_borrows 使用精确键（(pid, ResourceId), ResourceId）
//! - 借用：资源级（shared/exclusive + freeze），作用域包含规则（borrow_scope ⊆ owner_scope）
//! - revoke：DFS 子→父；严格模式报错；延迟模式挂起，借用清零后自动完成
//! - RAII：进程/线程/系统调用作用域退出时按创建顺序逆序撤销（确定性 Drop 顺序）

use super::ProcessId;
use alloc::collections::{BTreeMap, BTreeSet};
use alloc::vec::Vec;
use core::marker::PhantomData;
use core::sync::atomic::{AtomicU32, AtomicU64, Ordering};
use spin::{Mutex, RwLock};

// ========== Per-CPU 缓存 ==========

const MAX_CPUS: usize = 256;

#[repr(align(64))]
struct PerCpuCache {
    recent_caps: [AtomicU32; 16], // 保存表索引
    hits: AtomicU64,
    misses: AtomicU64,
}
impl PerCpuCache {
    const fn new() -> Self {
        const INV: AtomicU32 = AtomicU32::new(u32::MAX);
        Self { recent_caps: [INV; 16], hits: AtomicU64::new(0), misses: AtomicU64::new(0) }
    }
    #[inline(always)]
    fn slot(&self, pid: u32, rid_hash: u64) -> usize {
        let h = (pid as u64).wrapping_mul(0x9e3779b97f4a7c15) ^ rid_hash;
        (h as usize) & 15
    }
    fn lookup_validated(&self, pid: u32, rid: &ResourceId) -> Option<u32> {
        let s = self.slot(pid, rid.fast_hash());
        let idx = self.recent_caps[s].load(Ordering::Relaxed);
        if idx == u32::MAX { self.misses.fetch_add(1, Ordering::Relaxed); return None; }
        let ro = RO_DATA.read();
        if let Some(e) = ro.get(idx as usize) {
            if e.state == SlotState::Live && e.owner_pid == pid && e.resource_id == *rid {
                self.hits.fetch_add(1, Ordering::Relaxed);
                return Some(idx);
            }
        }
        self.misses.fetch_add(1, Ordering::Relaxed);
        None
    }
    fn insert(&self, pid: u32, rid_hash: u64, idx: u32) {
        let s = self.slot(pid, rid_hash);
        self.recent_caps[s].store(idx, Ordering::Relaxed);
    }
    fn invalidate_idx(&self, idx: u32) {
        for i in 0..16 {
            if self.recent_caps[i].load(Ordering::Relaxed) == idx {
                self.recent_caps[i].store(u32::MAX, Ordering::Relaxed);
            }
        }
    }
}
static PER_CPU: [PerCpuCache; MAX_CPUS] = {
    const C: PerCpuCache = PerCpuCache::new();
    [C; MAX_CPUS]
};
#[inline(always)]
fn cpu_id() -> usize { 0 } // 按需实现真实 CPU ID
fn pcache_invalidate_all(idx: u32) { for c in &PER_CPU { c.invalidate_idx(idx); } }

// ========== 能力与资源定义 ==========

pub mod caps {
    pub const READ: u32 = 1 << 0;
    pub const WRITE: u32 = 1 << 1;
    pub const EXECUTE: u32 = 1 << 2;
    pub const MAP: u32 = 1 << 3;
    pub const DELETE: u32 = 1 << 4;
    pub const TRANSFER: u32 = 1 << 5;
    pub const GRANT: u32 = 1 << 6;
    pub const REVOKE: u32 = 1 << 7;
    pub const ALL: u32 = 0xFF;
    pub const RW: u32 = READ | WRITE;
    pub const RO: u32 = READ;
    pub const TRANSFERABLE_MASK: u32 = READ | WRITE | EXECUTE | MAP | DELETE;
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[repr(u8)]
pub enum ResourceType {
    PhysicalPage = 0,
    VirtualMemory = 1,
    IoPort = 2,
    Interrupt = 3,
    DmaChannel = 4,
    Device = 5,
    IpcChannel = 6,
    Custom = 255,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[repr(C)]
pub struct ResourceId {
    id: u64,
    typ: ResourceType,
}
impl ResourceId {
    pub fn new(typ: ResourceType, id: u64) -> Self { Self { id, typ } }
    pub fn resource_type(&self) -> ResourceType { self.typ }
    pub fn id(&self) -> u64 { self.id }
    pub fn from_page_addr(addr: usize) -> Self { Self::new(ResourceType::PhysicalPage, addr as u64) }
    pub fn from_interrupt(irq: u8) -> Self { Self::new(ResourceType::Interrupt, irq as u64) }
    pub fn from_io_port(port: u16) -> Self { Self::new(ResourceType::IoPort, port as u64) }
    #[inline(always)]
    pub fn fast_hash(&self) -> u64 { self.id.wrapping_mul(0x9e3779b97f4a7c15) ^ (self.typ as u64) }
}

pub mod access {
    pub struct ReadOnly;
    pub struct Exclusive;
    pub struct FrozenShared;
}
pub mod lifetime {
    use core::marker::PhantomData;
    pub struct Permanent; pub struct Process; pub struct Thread; pub struct Syscall;
    pub struct Scoped<L>(pub PhantomData<L>);
    impl<L> Scoped<L> { pub const fn new() -> Self { Self(PhantomData) } }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct ThreadId(u64);
impl ThreadId { pub fn new(id: u64) -> Self { Self(id) } pub fn as_u64(self) -> u64 { self.0 } }

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum ScopeKind {
    Syscall(ThreadId, u64),
    Thread(ThreadId),
    Process,
    Permanent,
}
impl ScopeKind {
    #[inline(always)]
    fn can_borrow_from(&self, owner: &ScopeKind) -> bool {
        match (self, owner) {
            (_, ScopeKind::Permanent) => true,
            (_, ScopeKind::Process) => true,
            (ScopeKind::Thread(a), ScopeKind::Thread(b)) => a == b,
            (ScopeKind::Syscall(a, _), ScopeKind::Thread(b)) => a == b,
            (ScopeKind::Syscall(a, sa), ScopeKind::Syscall(b, sb)) => a == b && sa == sb,
            _ => false,
        }
    }
}

// ========== 句柄与表项 ==========

#[derive(Debug)]
#[repr(C, align(8))]
pub struct CapabilityHandle<Access = access::ReadOnly, Scope = lifetime::Permanent> {
    index_gen: u64, // index(32) | generation(32)
    scope: ScopeKind,
    creation_order: u64,
    _phantom: PhantomData<(Access, Scope)>,
}
impl<A, S> CapabilityHandle<A, S> {
    #[inline(always)]
    fn new(index: u32, generation: u32, scope: ScopeKind, creation_order: u64) -> Self {
        Self { index_gen: ((generation as u64) << 32) | (index as u64), scope, creation_order, _phantom: PhantomData }
    }
    #[inline(always)] fn index(&self) -> u32 { self.index_gen as u32 }
    #[inline(always)] fn generation(&self) -> u32 { (self.index_gen >> 32) as u32 }
    pub fn as_raw(&self) -> (u32, u32) { (self.index(), self.generation()) }
}
impl CapabilityHandle<access::Exclusive> {
    pub fn freeze(&self) -> CapabilityHandle<access::FrozenShared> {
        CapabilityHandle { index_gen: self.index_gen, scope: self.scope, creation_order: self.creation_order, _phantom: PhantomData }
    }
    pub fn downgrade(self) -> CapabilityHandle<access::ReadOnly> {
        CapabilityHandle { index_gen: self.index_gen, scope: self.scope, creation_order: self.creation_order, _phantom: PhantomData }
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum SlotState { Free = 0, Allocating = 1, Live = 2, PendingRevoke = 3 }

#[derive(Clone, Copy)]
#[repr(C, align(64))]
struct CapabilityEntry {
    // 32B
    resource_id: ResourceId,
    owner_pid: u32,
    capabilities: u32,
    generation: u32,
    state: SlotState,
    _pad_cc: u8,           // reserved
    _pad1: [u8; 7],
    // 32B
    created_at: u64,
    creation_order: u64,
    scope: ScopeKind,
}
impl CapabilityEntry {
    const fn empty() -> Self {
        Self {
            resource_id: ResourceId { id: 0, typ: ResourceType::Custom },
            owner_pid: 0, capabilities: 0, generation: 0,
            state: SlotState::Free, _pad_cc: 0, _pad1: [0; 7],
            created_at: 0, creation_order: 0, scope: ScopeKind::Permanent,
        }
    }
}

const MAX_CAPABILITIES: usize = 8192;

// 真源：只读表
static RO_DATA: RwLock<[CapabilityEntry; MAX_CAPABILITIES]> =
    RwLock::new([CapabilityEntry::empty(); MAX_CAPABILITIES]);

// 写入侧索引等
struct WriteData {
    free_slots: Vec<u32>,
    quick_cache: BTreeMap<(u32, ResourceId), Vec<u32>>, // (pid, rid) -> indices
    process_caps: BTreeMap<u32, Vec<u32>>,
    thread_caps: BTreeMap<u64, Vec<u32>>,
    syscall_caps: BTreeMap<(u64, u64), Vec<u32>>,
    // 授权树关系（父→子，子→父）
    children_of: BTreeMap<u32, Vec<u32>>,
    parent_of: BTreeMap<u32, u32>,
    // 借用状态（资源级）与延迟撤销列表
    resource_borrows: BTreeMap<ResourceId, ResourceBorrowState>,
    pending_revoke: BTreeMap<ResourceId, Vec<u32>>, // resource -> indices pending
    used_count: u32,
}
static WR_DATA: Mutex<WriteData> = Mutex::new(WriteData {
    free_slots: Vec::new(),
    quick_cache: BTreeMap::new(),
    process_caps: BTreeMap::new(),
    thread_caps: BTreeMap::new(),
    syscall_caps: BTreeMap::new(),
    children_of: BTreeMap::new(),
    parent_of: BTreeMap::new(),
    resource_borrows: BTreeMap::new(),
    pending_revoke: BTreeMap::new(),
    used_count: 0,
});

static GLOBAL_TIMESTAMP: AtomicU64 = AtomicU64::new(0);
static CREATION_SEQ: AtomicU64 = AtomicU64::new(0);

// ========== 借用状态（资源级） ==========

#[derive(Debug, Clone)]
struct ResourceBorrowState {
    shared: Vec<(u32, ThreadId)>,               // (cap_idx, tid)
    exclusive: Option<(u32, ThreadId, ScopeKind)>,
    frozen_count: u32,                           // 仅允许 exclusive 持有者线程 reborrow 为 &T
}
impl ResourceBorrowState {
    fn new() -> Self { Self { shared: Vec::new(), exclusive: None, frozen_count: 0 } }
    fn has_active(&self) -> bool {
        self.exclusive.is_some() || !self.shared.is_empty() || self.frozen_count > 0
    }
    fn can_revoke(&self) -> bool { !self.has_active() }
    fn try_shared(&mut self, cap_idx: u32, tid: ThreadId, caps_bits: u32) -> Result<(), CapError> {
        if (caps_bits & caps::READ) == 0 { return Err(CapError::PermissionDenied); }
        if let Some((_, ex_tid, _)) = self.exclusive {
            // 允许冻结场景下的同线程只读借用
            if self.frozen_count == 0 || ex_tid != tid { return Err(CapError::BorrowConflict); }
        }
        if self.shared.iter().any(|(i, t)| *i == cap_idx && *t == tid) {
            return Err(CapError::AlreadyBorrowed);
        }
        if self.shared.len() >= u16::MAX as usize { return Err(CapError::TooManyBorrows); }
        self.shared.push((cap_idx, tid));
        Ok(())
    }
    fn try_exclusive(&mut self, cap_idx: u32, tid: ThreadId, scope: ScopeKind, caps_bits: u32, rty: ResourceType)
                     -> Result<(), CapError> {
        let req = match rty { ResourceType::PhysicalPage|ResourceType::VirtualMemory => caps::WRITE|caps::MAP,
            ResourceType::Device|ResourceType::IoPort => caps::WRITE,
            _ => caps::WRITE };
        if (caps_bits & req) != req { return Err(CapError::PermissionDenied); }
        if self.exclusive.is_some() || !self.shared.is_empty() || self.frozen_count > 0 {
            return Err(CapError::BorrowConflict);
        }
        self.exclusive = Some((cap_idx, tid, scope));
        Ok(())
    }
    fn release_shared(&mut self, cap_idx: u32, tid: ThreadId) -> Result<(), CapError> {
        if let Some(pos) = self.shared.iter().position(|(i,t)| *i == cap_idx && *t == tid) {
            self.shared.swap_remove(pos);
            Ok(())
        } else { Err(CapError::NotBorrowed) }
    }
    fn release_exclusive(&mut self, cap_idx: u32, tid: ThreadId) -> Result<(), CapError> {
        match self.exclusive {
            Some((i, t, _)) if i == cap_idx && t == tid => {
                if self.frozen_count > 0 { return Err(CapError::StillFrozen); }
                self.exclusive = None;
                Ok(())
            }
            _ => Err(CapError::NotBorrowed)
        }
    }
    fn freeze(&mut self, cap_idx: u32, tid: ThreadId) -> Result<(), CapError> {
        match self.exclusive {
            Some((i, t, _)) if i == cap_idx && t == tid => { self.frozen_count = self.frozen_count.saturating_add(1); Ok(()) }
            _ => Err(CapError::NotBorrowed)
        }
    }
    fn unfreeze(&mut self, cap_idx: u32, tid: ThreadId) -> Result<(), CapError> {
        match self.exclusive {
            Some((i, t, _)) if i == cap_idx && t == tid => {
                if self.frozen_count == 0 { return Err(CapError::NotFrozen); }
                self.frozen_count -= 1; Ok(())
            }
            _ => Err(CapError::NotBorrowed)
        }
    }
}

// ========== 错误 ==========

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CapError {
    TableFull,
    PermissionDenied,
    ResourceNotFound,
    InvalidHandle,
    AlreadyBound,
    Unsupported,
    TooManyChildren,
    Expired,
    BorrowConflict,
    TooManyBorrows,
    NotBorrowed,
    AlreadyBorrowed,
    StillFrozen,
    NotFrozen,
}

// ========== 初始化 ==========

pub fn init() {
    let mut wr = WR_DATA.lock();
    wr.free_slots.clear();
    wr.free_slots.reserve(MAX_CAPABILITIES);
    for i in (0..MAX_CAPABILITIES).rev() { wr.free_slots.push(i as u32); }
    wr.quick_cache.clear();
    wr.process_caps.clear();
    wr.thread_caps.clear();
    wr.syscall_caps.clear();
    wr.children_of.clear();
    wr.parent_of.clear();
    wr.resource_borrows.clear();
    wr.pending_revoke.clear();
    wr.used_count = 0;

    let mut ro = RO_DATA.write();
    *ro = [CapabilityEntry::empty(); MAX_CAPABILITIES];
}

// ========== 工具：验证 & 释放 & 索引更新 ==========

#[inline(always)]
fn fast_validate<A, S>(h: &CapabilityHandle<A, S>) -> Result<(), CapError> {
    let idx = h.index() as usize;
    if idx >= MAX_CAPABILITIES { return Err(CapError::InvalidHandle); }
    let ro = RO_DATA.read();
    let e = &ro[idx];
    if e.state != SlotState::Live { return Err(CapError::InvalidHandle); }
    if e.generation != h.generation() { return Err(CapError::InvalidHandle); }
    if e.scope != h.scope { return Err(CapError::InvalidHandle); }
    Ok(())
}

fn qc_remove_idx(wr: &mut WriteData, pid: u32, rid: ResourceId, idx: u32) {
    if let Some(v) = wr.quick_cache.get_mut(&(pid, rid)) {
        v.retain(|&x| x != idx);
        if v.is_empty() { wr.quick_cache.remove(&(pid, rid)); }
    }
}
fn scope_remove_idx(wr: &mut WriteData, scope: ScopeKind, idx: u32) {
    match scope {
        ScopeKind::Process => { /* 无法仅凭 scope 移除，需要 owner_pid；调用处处理 */ }
        ScopeKind::Thread(t) => if let Some(v)=wr.thread_caps.get_mut(&t.as_u64()){ v.retain(|&x|x!=idx); if v.is_empty(){wr.thread_caps.remove(&t.as_u64());}},
        ScopeKind::Syscall(t, s) => if let Some(v)=wr.syscall_caps.get_mut(&(t.as_u64(),s)){ v.retain(|&x|x!=idx); if v.is_empty(){wr.syscall_caps.remove(&(t.as_u64(),s));}},
        ScopeKind::Permanent => {}
    }
}

fn unlink_graph_locked(wr: &mut WriteData, idx: u32) {
    if let Some(p) = wr.parent_of.remove(&idx) {
        if let Some(children) = wr.children_of.get_mut(&p) {
            children.retain(|&c| c != idx);
            if children.is_empty() { wr.children_of.remove(&p); }
        }
    }
    if let Some(children) = wr.children_of.remove(&idx) {
        for c in children {
            wr.parent_of.remove(&c);
        }
    }
}

fn free_slot_locked(wr: &mut WriteData, ro: &mut [CapabilityEntry; MAX_CAPABILITIES], idx: u32) {
    let e = &mut ro[idx as usize];
    e.generation = e.generation.wrapping_add(1);
    e.state = SlotState::Free;
    wr.used_count = wr.used_count.saturating_sub(1);
    wr.free_slots.push(idx);
    pcache_invalidate_all(idx);
}

// 若资源无借用且未挂起，则立即撤销；否则严格/延迟策略
fn revoke_one_locked(
    wr: &mut WriteData,
    ro: &mut [CapabilityEntry; MAX_CAPABILITIES],
    idx: u32,
    strict: bool,
) -> Result<(), CapError> {
    let e = ro[idx as usize]; // copy
    let rid = e.resource_id;
    if let Some(bs) = wr.resource_borrows.get(&rid) {
        if bs.has_active() {
            if strict { return Err(CapError::BorrowConflict); }
            wr.pending_revoke.entry(rid).or_default().push(idx);
            ro[idx as usize].state = SlotState::PendingRevoke;
            return Ok(());
        }
    }
    // 真撤销
    qc_remove_idx(wr, e.owner_pid, e.resource_id, idx);
    scope_remove_idx(wr, e.scope, idx);
    unlink_graph_locked(wr, idx);
    free_slot_locked(wr, ro, idx);
    Ok(())
}

// DFS 撤销（先子后父）
fn revoke_dfs_locked(
    wr: &mut WriteData,
    ro: &mut [CapabilityEntry; MAX_CAPABILITIES],
    idx: u32,
    strict: bool,
) -> Result<(), CapError> {
    if (idx as usize) >= MAX_CAPABILITIES { return Ok(()); }
    if ro[idx as usize].state == SlotState::Free { return Ok(()); }

    let children = wr.children_of.get(&idx).cloned().unwrap_or_default();
    for c in children {
        revoke_dfs_locked(wr, ro, c, strict)?;
    }
    revoke_one_locked(wr, ro, idx, strict)
}

// 借用释放后尝试完成延迟撤销
fn try_complete_pending_for(wr: &mut WriteData, ro: &mut [CapabilityEntry; MAX_CAPABILITIES], rid: ResourceId) {
    if let Some(list) = wr.pending_revoke.get_mut(&rid) {
        // 先检查是否仍有活跃借用
        if let Some(bs) = wr.resource_borrows.get(&rid) {
            if bs.has_active() { return; }
        }
        let idxs = core::mem::take(list);
        for idx in idxs {
            let _ = revoke_one_locked(wr, ro, idx, true); // 现在应能立即撤销
        }
        wr.pending_revoke.remove(&rid);
    }
}

// ========== 绑定（只读 / 独占 / 指定作用域） ==========

pub fn bind_resource_readonly(pid: ProcessId, rid: ResourceId)
                              -> Result<CapabilityHandle<access::ReadOnly>, CapError>
{
    let creation = CREATION_SEQ.fetch_add(1, Ordering::Relaxed);
    if let Some(idx) = PER_CPU[cpu_id()].lookup_validated(pid.as_u32(), &rid) {
        let ro = RO_DATA.read(); let e = ro[idx as usize];
        return Ok(CapabilityHandle::new(idx, e.generation, e.scope, e.creation_order));
    }
    bind_internal::<access::ReadOnly, lifetime::Process>(pid, rid, caps::READ, ScopeKind::Process, creation, None)
}

pub fn bind_resource_exclusive(pid: ProcessId, rid: ResourceId)
                               -> Result<CapabilityHandle<access::Exclusive>, CapError>
{
    let creation = CREATION_SEQ.fetch_add(1, Ordering::Relaxed);
    bind_internal::<access::Exclusive, lifetime::Process>(pid, rid, caps::RW | caps::MAP, ScopeKind::Process, creation, None)
}

pub fn bind_resource_scoped<A,S>(
    pid: ProcessId, rid: ResourceId, caps_bits: u32, scope: ScopeKind,
) -> Result<CapabilityHandle<A,S>, CapError> {
    let creation = CREATION_SEQ.fetch_add(1, Ordering::Relaxed);
    bind_internal::<A,S>(pid, rid, caps_bits, scope, creation, None)
}

// 内部绑定；可指定父节点（授权）
fn bind_internal<A,S>(
    pid: ProcessId, rid: ResourceId, caps_bits: u32, scope: ScopeKind, creation_order: u64, parent: Option<u32>,
) -> Result<CapabilityHandle<A,S>, CapError> {
    let mut wr = WR_DATA.lock();
    let key = (pid.as_u32(), rid);

    if let Some(indices) = wr.quick_cache.get(&key) {
        let ro = RO_DATA.read();
        for &idx in indices {
            let e = ro[idx as usize];
            if e.state == SlotState::Live && e.owner_pid == pid.as_u32() && e.resource_id == rid {
                // 可在此升级权限（需要 RO 写锁）——此处保持只读以避免竞态
                return Ok(CapabilityHandle::new(idx, e.generation, e.scope, e.creation_order));
            }
        }
    }

    let idx = wr.free_slots.pop().ok_or(CapError::TableFull)?;
    let ts = GLOBAL_TIMESTAMP.fetch_add(1, Ordering::Relaxed);

    {
        let mut ro = RO_DATA.write();
        let e = &mut ro[idx as usize];
        let gen = e.generation;
        *e = CapabilityEntry {
            resource_id: rid, owner_pid: pid.as_u32(), capabilities: caps_bits,
            generation: gen, state: SlotState::Live, _pad_cc: 0, _pad1: [0; 7],
            created_at: ts, creation_order, scope,
        };
    }

    wr.quick_cache.entry(key).or_default().push(idx);
    wr.used_count += 1;
    PER_CPU[cpu_id()].insert(pid.as_u32(), rid.fast_hash(), idx);

    wr.resource_borrows.entry(rid).or_insert_with(ResourceBorrowState::new);

    match scope {
        ScopeKind::Process => wr.process_caps.entry(pid.as_u32()).or_default().push(idx),
        ScopeKind::Thread(t) => wr.thread_caps.entry(t.as_u64()).or_default().push(idx),
        ScopeKind::Syscall(t,s) => wr.syscall_caps.entry((t.as_u64(),s)).or_default().push(idx),
        ScopeKind::Permanent => {}
    }

    if let Some(p) = parent {
        // 限制子节点数量
        let v = wr.children_of.entry(p).or_default();
        const MAX_CHILDREN_PER_CAP: usize = 32;
        if v.len() >= MAX_CHILDREN_PER_CAP { return Err(CapError::TooManyChildren); }
        v.push(idx);
        wr.parent_of.insert(idx, p);
    }

    let ro = RO_DATA.read(); let e = ro[idx as usize];
    Ok(CapabilityHandle::new(idx, e.generation, e.scope, e.creation_order))
}

// ========== 授权与转移 ==========

pub fn grant_readonly(
    grantor_pid: ProcessId, grantee_pid: ProcessId, rid: ResourceId
) -> Result<CapabilityHandle<access::ReadOnly>, CapError> {
    let mut wr = WR_DATA.lock();
    let key = (grantor_pid.as_u32(), rid);
    let (parent_idx, parent_caps) = {
        let ro = RO_DATA.read();
        let idxs = wr.quick_cache.get(&key).cloned().ok_or(CapError::ResourceNotFound)?;
        let mut found = None;
        for idx in idxs {
            let e = ro[idx as usize];
            if e.state == SlotState::Live && e.owner_pid == grantor_pid.as_u32() && e.resource_id == rid {
                if (e.capabilities & caps::GRANT) == 0 { return Err(CapError::PermissionDenied); }
                found = Some((idx, e.capabilities)); break;
            }
        }
        found.ok_or(CapError::ResourceNotFound)?
    };
    // 只能授予自己拥有且可传播的权限（这里授予只读）
    if (parent_caps & caps::READ) == 0 { return Err(CapError::PermissionDenied); }
    drop(wr);
    bind_internal::<access::ReadOnly, lifetime::Process>(
        grantee_pid, rid, caps::READ, ScopeKind::Process, CREATION_SEQ.fetch_add(1, Ordering::Relaxed), Some(parent_idx))
}

pub fn grant_exclusive(
    grantor_pid: ProcessId, grantee_pid: ProcessId, rid: ResourceId
) -> Result<CapabilityHandle<access::Exclusive>, CapError> {
    let mut wr = WR_DATA.lock();
    let key = (grantor_pid.as_u32(), rid);
    let (parent_idx, parent_caps) = {
        let ro = RO_DATA.read();
        let idxs = wr.quick_cache.get(&key).cloned().ok_or(CapError::ResourceNotFound)?;
        let mut found = None;
        for idx in idxs {
            let e = ro[idx as usize];
            if e.state == SlotState::Live && e.owner_pid == grantor_pid.as_u32() && e.resource_id == rid {
                if (e.capabilities & caps::GRANT) == 0 { return Err(CapError::PermissionDenied); }
                found = Some((idx, e.capabilities)); break;
            }
        }
        found.ok_or(CapError::ResourceNotFound)?
    };
    let grantable = parent_caps & caps::TRANSFERABLE_MASK;
    if (grantable & (caps::RW)) != (caps::RW) { return Err(CapError::PermissionDenied); }
    drop(wr);
    bind_internal::<access::Exclusive, lifetime::Process>(
        grantee_pid, rid, caps::RW | caps::MAP, ScopeKind::Process, CREATION_SEQ.fetch_add(1, Ordering::Relaxed), Some(parent_idx))
}

pub fn transfer_resource(
    from_pid: ProcessId, to_pid: ProcessId, rid: ResourceId
) -> Result<(), CapError> {
    let mut wr = WR_DATA.lock();
    let key = (from_pid.as_u32(), rid);
    let idx = {
        let ro = RO_DATA.read();
        let idxs = wr.quick_cache.get(&key).cloned().ok_or(CapError::ResourceNotFound)?;
        let mut found = None;
        for i in idxs {
            let e = ro[i as usize];
            if e.state == SlotState::Live && e.owner_pid == from_pid.as_u32() && e.resource_id == rid {
                if (e.capabilities & caps::TRANSFER) == 0 { return Err(CapError::PermissionDenied); }
                found = Some(i); break;
            }
        }
        found.ok_or(CapError::ResourceNotFound)?
    };
    // 剥离管理权限
    let caps_new = {
        let ro = RO_DATA.read(); ro[idx as usize].capabilities & caps::TRANSFERABLE_MASK
    };
    {
        let mut ro = RO_DATA.write();
        revoke_dfs_locked(&mut wr, &mut ro, idx, true)?;
    }
    drop(wr);
    // 为新进程建立独立能力（根据新权限选择只读或独占）
    if (caps_new & (caps::WRITE|caps::MAP)) == (caps::WRITE|caps::MAP) {
        let _ = bind_internal::<access::Exclusive, lifetime::Process>(
            to_pid, rid, caps::RW | caps::MAP, ScopeKind::Process, CREATION_SEQ.fetch_add(1, Ordering::Relaxed), None)?;
    } else {
        let _ = bind_internal::<access::ReadOnly, lifetime::Process>(
            to_pid, rid, caps::READ, ScopeKind::Process, CREATION_SEQ.fetch_add(1, Ordering::Relaxed), None)?;
    }
    Ok(())
}

// ========== 借用 API（资源级） ==========

pub fn borrow_shared_ro(
    h: &CapabilityHandle<access::ReadOnly>, tid: ThreadId, borrow_scope: ScopeKind,
) -> Result<(), CapError> {
    fast_validate(h)?;
    let ro = RO_DATA.read();
    let e = ro[h.index() as usize];
    if !borrow_scope.can_borrow_from(&e.scope) { return Err(CapError::BorrowConflict); }
    drop(ro);
    let mut wr = WR_DATA.lock();
    let bs = wr.resource_borrows.get_mut(&e.resource_id).ok_or(CapError::ResourceNotFound)?;
    bs.try_shared(h.index(), tid, e.capabilities)
}

pub fn borrow_shared_from_frozen(
    h: &CapabilityHandle<access::FrozenShared>, tid: ThreadId, borrow_scope: ScopeKind,
) -> Result<(), CapError> {
    fast_validate(h)?;
    let ro = RO_DATA.read();
    let e = ro[h.index() as usize];
    if !borrow_scope.can_borrow_from(&e.scope) { return Err(CapError::BorrowConflict); }
    drop(ro);
    let mut wr = WR_DATA.lock();
    let bs = wr.resource_borrows.get_mut(&e.resource_id).ok_or(CapError::ResourceNotFound)?;
    // 允许共享借用；必须为同线程且已冻结（在 try_shared 中检查）
    bs.try_shared(h.index(), tid, e.capabilities)
}

pub fn borrow_exclusive(
    h: &CapabilityHandle<access::Exclusive>, tid: ThreadId, borrow_scope: ScopeKind,
) -> Result<(), CapError> {
    fast_validate(h)?;
    let ro = RO_DATA.read();
    let e = ro[h.index() as usize];
    if !borrow_scope.can_borrow_from(&e.scope) { return Err(CapError::BorrowConflict); }
    let rid = e.resource_id; let caps_bits = e.capabilities; let rty = e.resource_id.resource_type();
    drop(ro);
    let mut wr = WR_DATA.lock();
    let bs = wr.resource_borrows.get_mut(&rid).ok_or(CapError::ResourceNotFound)?;
    bs.try_exclusive(h.index(), tid, borrow_scope, caps_bits, rty)
}

pub fn release_shared(
    h: &CapabilityHandle<access::ReadOnly>, tid: ThreadId
) -> Result<(), CapError> {
    fast_validate(h)?;
    let e = { let ro=RO_DATA.read(); ro[h.index() as usize] };
    let mut wr = WR_DATA.lock();
    let bs = wr.resource_borrows.get_mut(&e.resource_id).ok_or(CapError::ResourceNotFound)?;
    bs.release_shared(h.index(), tid)?;
    // 尝试完成延迟撤销
    let mut ro = RO_DATA.write();
    try_complete_pending_for(&mut wr, &mut ro, e.resource_id);
    Ok(())
}

pub fn release_shared_frozen(
    h: &CapabilityHandle<access::FrozenShared>, tid: ThreadId
) -> Result<(), CapError> {
    fast_validate(h)?;
    let e = { let ro=RO_DATA.read(); ro[h.index() as usize] };
    let mut wr = WR_DATA.lock();
    let bs = wr.resource_borrows.get_mut(&e.resource_id).ok_or(CapError::ResourceNotFound)?;
    bs.release_shared(h.index(), tid)?;
    let mut ro = RO_DATA.write();
    try_complete_pending_for(&mut wr, &mut ro, e.resource_id);
    Ok(())
}

pub fn release_exclusive(
    h: &CapabilityHandle<access::Exclusive>, tid: ThreadId
) -> Result<(), CapError> {
    fast_validate(h)?;
    let e = { let ro=RO_DATA.read(); ro[h.index() as usize] };
    let mut wr = WR_DATA.lock();
    let bs = wr.resource_borrows.get_mut(&e.resource_id).ok_or(CapError::ResourceNotFound)?;
    bs.release_exclusive(h.index(), tid)?;
    let mut ro = RO_DATA.write();
    try_complete_pending_for(&mut wr, &mut ro, e.resource_id);
    Ok(())
}

pub fn freeze_exclusive(
    h: &CapabilityHandle<access::Exclusive>, tid: ThreadId
) -> Result<CapabilityHandle<access::FrozenShared>, CapError> {
    fast_validate(h)?;
    let e = { let ro=RO_DATA.read(); ro[h.index() as usize] };
    let mut wr = WR_DATA.lock();
    let bs = wr.resource_borrows.get_mut(&e.resource_id).ok_or(CapError::ResourceNotFound)?;
    bs.freeze(h.index(), tid)?;
    Ok(h.freeze())
}
pub fn unfreeze_exclusive(
    h: &CapabilityHandle<access::Exclusive>, tid: ThreadId
) -> Result<(), CapError> {
    fast_validate(h)?;
    let e = { let ro=RO_DATA.read(); ro[h.index() as usize] };
    let mut wr = WR_DATA.lock();
    let bs = wr.resource_borrows.get_mut(&e.resource_id).ok_or(CapError::ResourceNotFound)?;
    bs.unfreeze(h.index(), tid)
}

// ========== 撤销（严格/延迟） ==========

pub fn revoke_capability<A,S>(h: &CapabilityHandle<A,S>) -> Result<(), CapError> {
    fast_validate(h)?;
    let mut wr = WR_DATA.lock();
    let mut ro = RO_DATA.write();
    revoke_dfs_locked(&mut wr, &mut ro, h.index(), true)
}

pub fn revoke_capability_deferred<A,S>(h: &CapabilityHandle<A,S>) -> Result<(), CapError> {
    fast_validate(h)?;
    let mut wr = WR_DATA.lock();
    let mut ro = RO_DATA.write();
    revoke_dfs_locked(&mut wr, &mut ro, h.index(), false)
}

// ========== 验证（快路径 + 回退） ==========

#[inline]
pub fn verify_capability_fast(pid: ProcessId, rid: ResourceId, required: u32) -> bool {
    if let Some(idx) = PER_CPU[cpu_id()].lookup_validated(pid.as_u32(), &rid) {
        let ro = RO_DATA.read(); let e = ro[idx as usize];
        return (e.capabilities & required) == required;
    }
    false
}
pub fn verify_capability(pid: ProcessId, rid: ResourceId, required: u32) -> bool {
    if verify_capability_fast(pid, rid, required) { return true; }
    {
        let wr = WR_DATA.lock();
        if let Some(indices) = wr.quick_cache.get(&(pid.as_u32(), rid)) {
            let ro = RO_DATA.read();
            for &idx in indices {
                let e = ro[idx as usize];
                if e.state == SlotState::Live && e.owner_pid == pid.as_u32() && e.resource_id == rid
                    && (e.capabilities & required) == required { return true; }
            }
        }
    }
    let ro = RO_DATA.read();
    for e in ro.iter() {
        if e.state == SlotState::Live && e.owner_pid == pid.as_u32() && e.resource_id == rid
            && (e.capabilities & required) == required { return true; }
    }
    false
}

// ========== RAII 作用域回收（确定性 Drop） ==========

fn revoke_indices_deterministic(mut idxs: Vec<u32>) -> usize {
    // 读取创建序并按逆序撤销（Rust 的 Drop 顺序）
    {
        let ro = RO_DATA.read();
        idxs.sort_by_key(|&i| core::cmp::Reverse(ro[i as usize].creation_order));
    }
    let mut wr = WR_DATA.lock();
    let mut ro = RO_DATA.write();
    let mut count = 0usize;
    for idx in idxs {
        if ro[idx as usize].state != SlotState::Free {
            if revoke_dfs_locked(&mut wr, &mut ro, idx, true).is_ok() { count += 1; }
        }
    }
    count
}

pub fn on_process_exit(pid: ProcessId) -> usize {
    let mut wr = WR_DATA.lock();
    let idxs = wr.process_caps.remove(&pid.as_u32()).unwrap_or_default();
    drop(wr);
    revoke_indices_deterministic(idxs)
}
pub fn on_thread_exit(tid: ThreadId) -> usize {
    let mut wr = WR_DATA.lock();
    let idxs = wr.thread_caps.remove(&tid.as_u64()).unwrap_or_default();
    drop(wr);
    revoke_indices_deterministic(idxs)
}
pub fn on_syscall_return(tid: ThreadId, seq: u64) -> usize {
    let mut wr = WR_DATA.lock();
    let idxs = wr.syscall_caps.remove(&(tid.as_u64(), seq)).unwrap_or_default();
    drop(wr);
    revoke_indices_deterministic(idxs)
}

// ========== 统计 ==========

pub struct CapabilityStats {
    pub total_slots: usize,
    pub used_slots: usize,
    pub free_slots: usize,
    pub cache_hits: u64,
    pub cache_misses: u64,
    pub cache_hit_rate: f32,
}
pub fn get_stats() -> CapabilityStats {
    let wr = WR_DATA.lock();
    let mut hits = 0u64; let mut misses = 0u64;
    for c in &PER_CPU { hits += c.hits.load(Ordering::Relaxed); misses += c.misses.load(Ordering::Relaxed); }
    let tot = hits + misses;
    CapabilityStats {
        total_slots: MAX_CAPABILITIES,
        used_slots: wr.used_count as usize,
        free_slots: MAX_CAPABILITIES - wr.used_count as usize,
        cache_hits: hits, cache_misses: misses,
        cache_hit_rate: if tot>0 { (hits as f32 / tot as f32)*100.0 } else { 0.0 },
    }
}
