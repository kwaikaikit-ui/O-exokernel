// src/capability/mod.rs
//! 能力和权限管理系统

pub mod resource;

use core::sync::atomic::{AtomicU32, Ordering};

/// 进程ID（类型安全）
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ProcessId(u32);

impl ProcessId {
    pub fn new(id: u32) -> Self {
        Self(id)
    }

    pub fn as_u32(self) -> u32 {
        self.0
    }
}

static NEXT_PID: AtomicU32 = AtomicU32::new(1);

pub fn init() {
    NEXT_PID.store(1, Ordering::Release);
}

pub fn allocate_pid() -> ProcessId {
    let pid = NEXT_PID.fetch_add(1, Ordering::AcqRel);
    ProcessId(pid)
}