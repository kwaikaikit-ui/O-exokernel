//! 设备树(DTB)解析 - 用于ARM/RISC-V

use alloc::vec::Vec;
use super::MemoryRegion;

const FDT_MAGIC: u32 = 0xd00dfeed;
const FDT_BEGIN_NODE: u32 = 0x00000001;
const FDT_END_NODE: u32 = 0x00000002;
const FDT_PROP: u32 = 0x00000003;
const FDT_END: u32 = 0x00000009;

pub fn parse(dtb_addr: *const u8) -> Vec<MemoryRegion> {
    let mut regions = Vec::new();

    unsafe {
        let magic = u32::from_be(*(dtb_addr as *const u32));
        if magic != FDT_MAGIC {
            crate::println!("  [DTB] Invalid magic: 0x{:x}", magic);
            return regions;
        }

        crate::println!("  [DTB] Valid device tree at {:p}", dtb_addr);

        let totalsize = u32::from_be(*(dtb_addr.add(4) as *const u32));
        let off_struct = u32::from_be(*(dtb_addr.add(8) as *const u32));

        parse_memory_node(dtb_addr, off_struct as usize, &mut regions);
    }

    regions
}

unsafe fn parse_memory_node(
    dtb: *const u8,
    struct_offset: usize,
    regions: &mut Vec<MemoryRegion>
) {
    // 简化实现：查找/memory节点
    // 完整实现需要遍历整个FDT结构

    // 默认返回一些合理的内存区域（针对常见ARM/RISC-V板子）
    regions.push(MemoryRegion {
        base: 0x80000000, // RISC-V/ARM常见起始地址
        size: 256 * 1024 * 1024, // 256MB
        available: true,
    });

    crate::println!("  [DTB] Default memory: 0x80000000 + 256MB");
}
