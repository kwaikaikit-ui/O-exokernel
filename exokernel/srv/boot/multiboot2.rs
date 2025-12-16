// src/boot/multiboot2.rs
//! 解析 Multiboot2 引导信息

use alloc::vec::Vec;
use super::MemoryRegion;

const MULTIBOOT2_TAG_END: u32 = 0;
const MULTIBOOT2_TAG_MMAP: u32 = 6;
const MULTIBOOT2_TAG_BOOTLOADER_NAME: u32 = 2;

#[repr(C)]
struct Multiboot2Tag {
    typ: u32,
    size: u32,
}

#[repr(C)]
struct Multiboot2MmapEntry {
    base_addr: u64,
    length: u64,
    typ: u32,
    _reserved: u32,
}

pub fn parse(info_addr: *const u8) -> Vec<MemoryRegion> {
    let mut regions = Vec::new();

    unsafe {
        let total_size = *(info_addr as *const u32);
        let mut tag_addr = info_addr.add(8); // 跳过总大小和保留字段
        let end_addr = info_addr.add(total_size as usize);

        while tag_addr < end_addr {
            let tag = &*(tag_addr as *const Multiboot2Tag);

            if tag.typ == MULTIBOOT2_TAG_END {
                break;
            }

            if tag.typ == MULTIBOOT2_TAG_MMAP {
                parse_memory_map(tag_addr, &mut regions);
            }

            if tag.typ == MULTIBOOT2_TAG_BOOTLOADER_NAME {
                let name_ptr = tag_addr.add(8);
                crate::println!("  [BOOT] Bootloader: {}",
                                core::str::from_utf8_unchecked(
                                    core::slice::from_raw_parts(name_ptr, 32)
                                ).trim_end_matches('\0'));
            }

            // 对齐到 8 字节
            tag_addr = tag_addr.add(((tag.size + 7) & !7) as usize);
        }
    }

    regions
}

unsafe fn parse_memory_map(tag_addr: *const u8, regions: &mut Vec<MemoryRegion>) {
    let entry_size = *(tag_addr.add(8) as *const u32);
    let entry_version = *(tag_addr.add(12) as *const u32);

    let mut entry_addr = tag_addr.add(16);
    let tag_size = *(tag_addr.add(4) as *const u32);
    let end_addr = tag_addr.add(tag_size as usize);

    while entry_addr < end_addr {
        let entry = &*(entry_addr as *const Multiboot2MmapEntry);

        if entry.typ == 1 && entry.length > 0 {
            regions.push(MemoryRegion {
                base: entry.base_addr as usize,
                size: entry.length as usize,
                available: true,
            });

            crate::println!("  [MEM] 0x{:016x} - 0x{:016x} ({}MB)",
                            entry.base_addr,
                            entry.base_addr + entry.length,
                            entry.length / (1024 * 1024));
        }

        entry_addr = entry_addr.add(entry_size as usize);
    }
}
