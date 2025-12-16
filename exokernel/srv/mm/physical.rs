// src/mm/physical.rs
//! 底层物理内存分配器

use core::sync::atomic::{AtomicUsize, Ordering};
use crate::arch::PAGE_SIZE;

const MAX_PAGES: usize = 65536;
const BITMAP_SIZE: usize = MAX_PAGES / 64;

struct PhysicalAllocator {
    base: usize,
    total_pages: usize,
    free_pages: AtomicUsize,
    bitmap: [AtomicUsize; BITMAP_SIZE],
    owners: [AtomicU32; MAX_PAGES],
}

struct AtomicU32(core::sync::atomic::AtomicU32);

unsafe impl Sync for PhysicalAllocator {}

static mut ALLOCATOR: PhysicalAllocator = PhysicalAllocator {
    base: 0,
    total_pages: 0,
    free_pages: AtomicUsize::new(0),
    bitmap: [const { AtomicUsize::new(0) }; BITMAP_SIZE],
    owners: [const { AtomicU32(core::sync::atomic::AtomicU32::new(0)) }; MAX_PAGES],
};

pub unsafe fn init(base: usize, size: usize) {
    ALLOCATOR.base = base;
    ALLOCATOR.total_pages = (size / PAGE_SIZE).min(MAX_PAGES);
    ALLOCATOR.free_pages.store(ALLOCATOR.total_pages, Ordering::Release);

    for i in 0..BITMAP_SIZE {
        ALLOCATOR.bitmap[i].store(0, Ordering::Release);
    }

    for i in 0..MAX_PAGES {
        ALLOCATOR.owners[i].0.store(0, Ordering::Release);
    }
}

pub unsafe fn alloc_raw(pid: u32) -> Option<usize> {
    let allocator = &ALLOCATOR;

    for word_idx in 0..BITMAP_SIZE {
        let mut word = allocator.bitmap[word_idx].load(Ordering::Acquire);

        while word != usize::MAX {
            for bit in 0..64 {
                if (word & (1 << bit)) == 0 {
                    let new_word = word | (1 << bit);

                    match allocator.bitmap[word_idx].compare_exchange(
                        word,
                        new_word,
                        Ordering::AcqRel,
                        Ordering::Acquire
                    ) {
                        Ok(_) => {
                            let page_idx = word_idx * 64 + bit;
                            if page_idx >= allocator.total_pages {
                                return None;
                            }

                            allocator.owners[page_idx].0.store(pid, Ordering::Release);
                            allocator.free_pages.fetch_sub(1, Ordering::AcqRel);

                            return Some(allocator.base + page_idx * PAGE_SIZE);
                        }
                        Err(current) => {
                            word = current;
                            break;
                        }
                    }
                }
            }

            if word == usize::MAX {
                break;
            }
        }
    }

    None
}

pub unsafe fn free_raw(pid: u32, addr: usize) -> Result<(), &'static str> {
    let allocator = &ALLOCATOR;

    if addr < allocator.base {
        return Err("Invalid address");
    }

    let page_idx = (addr - allocator.base) / PAGE_SIZE;
    if page_idx >= allocator.total_pages {
        return Err("Page index out of range");
    }

    let owner = allocator.owners[page_idx].0.load(Ordering::Acquire);
    if owner != pid {
        return Err("Permission denied");
    }

    allocator.owners[page_idx].0.store(0, Ordering::Release);

    let word_idx = page_idx / 64;
    let bit = page_idx % 64;

    allocator.bitmap[word_idx].fetch_and(!(1 << bit), Ordering::AcqRel);
    allocator.free_pages.fetch_add(1, Ordering::AcqRel);

    Ok(())
}

pub unsafe fn change_owner(addr: usize, old_pid: u32, new_pid: u32) -> Result<(), &'static str> {
    let allocator = &ALLOCATOR;
    let page_idx = (addr - allocator.base) / PAGE_SIZE;

    match allocator.owners[page_idx].0.compare_exchange(
        old_pid,
        new_pid,
        Ordering::AcqRel,
        Ordering::Acquire
    ) {
        Ok(_) => Ok(()),
        Err(_) => Err("Owner mismatch"),
    }
}

pub unsafe fn free_pages() -> usize {
    ALLOCATOR.free_pages.load(Ordering::Acquire)
}
