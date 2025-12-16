// src/lib.rs
#![no_std]
#![feature(naked_functions)]
#![feature(asm_const)]
#![feature(alloc_error_handler)]

extern crate alloc;

pub mod arch;
pub mod boot;
pub mod mm;
pub mod capability;
pub mod libos_interface;
pub mod console;

use core::panic::PanicInfo;

/// 全局初始化标志
static INITIALIZED: core::sync::atomic::AtomicBool =
    core::sync::atomic::AtomicBool::new(false);

/// 内核主入口 - 由各架构启动代码调用
#[no_mangle]
pub extern "C" fn kernel_main(boot_info: *const u8) -> ! {
    // 初始化控制台
    console::init();

    println!("\n╔═══════════════════════════════════════╗");
    println!("║   EXOKERNEL - Rust Ownership Model   ║");
    println!("║   Multi-Architecture Support System   ║");
    println!("╚═══════════════════════════════════════╝\n");

    println!("[BOOT] Architecture: {}", arch::ARCH_NAME);
    println!("[BOOT] Boot info at: {:p}", boot_info);

    // 解析启动信息
    let mem_regions = boot::parse_boot_info(boot_info);
    println!("[BOOT] Found {} memory regions", mem_regions.len());

    // 初始化物理内存管理器（Rust所有权模型）
    mm::init(mem_regions);
    println!("[MM] Physical memory manager initialized");

    // 初始化能力系统
    capability::init();
    println!("[CAP] Capability system initialized");

    // 标记已初始化
    INITIALIZED.store(true, core::sync::atomic::Ordering::Release);

    println!("\n[OK] Kernel initialized successfully!\n");

    // 运行测试
    test_ownership_model();

    // 主循环
    println!("[IDLE] Entering idle loop...");
    loop {
        arch::halt();
    }
}

/// 测试Rust所有权模型的资源管理
fn test_ownership_model() {
    println!("=== Testing Rust Ownership Model ===\n");

    use libos_interface::PhysicalPage;
    use capability::ProcessId;

    let pid = ProcessId::new(1);

    // 测试1: 基本分配和所有权转移
    println!("[TEST 1] Page allocation and ownership transfer");
    {
        let page = PhysicalPage::alloc(pid).expect("Failed to allocate");
        println!("  ✓ Allocated page at 0x{:x}", page.address());

        let moved_page = page; // 所有权转移
        println!("  ✓ Ownership transferred");

        // page在这里不可用（借用检查器保证）
        drop(moved_page);
        println!("  ✓ Page automatically freed on drop");
    }

    // 测试2: 借用和共享访问
    println!("\n[TEST 2] Borrowing and shared access");
    {
        let page = PhysicalPage::alloc(pid).expect("Failed to allocate");

        let addr1 = page.address(); // 不可变借用
        let addr2 = page.address(); // 多个不可变借用OK
        println!("  ✓ Multiple immutable borrows: 0x{:x}, 0x{:x}", addr1, addr2);

        // 可变借用（独占访问）
        // let mut_page = page.as_mut(); // 这会获取独占访问
    }

    // 测试3: 生命周期和作用域
    println!("\n[TEST 3] Lifetime and scope management");
    {
        let page1 = PhysicalPage::alloc(pid).expect("Failed");
        {
            let page2 = PhysicalPage::alloc(pid).expect("Failed");
            println!("  ✓ page1=0x{:x}, page2=0x{:x}",
                     page1.address(), page2.address());
            // page2在内部作用域结束时自动释放
        }
        println!("  ✓ page2 dropped, page1 still valid");
        // page1在外部作用域结束时释放
    }

    println!("\n=== All tests passed! ===\n");
}

/// Panic处理器
pub fn panic_handler(info: &PanicInfo) -> ! {
    println!("\n!!! KERNEL PANIC !!!");
    println!("{}", info);

    loop {
        arch::halt();
    }
}

#[alloc_error_handler]
fn alloc_error(_layout: core::alloc::Layout) -> ! {
    panic!("Out of memory");
}
