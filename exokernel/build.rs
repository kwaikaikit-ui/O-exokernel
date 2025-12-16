// build.rs
//! 构建脚本 - 处理架构特定的链接和配置

use std::env;
use std::path::PathBuf;

fn main() {
    // 获取目标架构
    let target = env::var("TARGET").unwrap();
    let out_dir = PathBuf::from(env::var("OUT_DIR").unwrap());

    println!("cargo:rerun-if-changed=build.rs");
    println!("cargo:rerun-if-changed=linker/");

    // 根据目标架构选择链接脚本
    let linker_script = match target.as_str() {
        "x86_64-unknown-none" => {
            println!("cargo:rerun-if-changed=linker/x86_64.ld");
            "linker/x86_64.ld"
        }
        "aarch64-unknown-none" => {
            println!("cargo:rerun-if-changed=linker/aarch64.ld");
            "linker/aarch64.ld"
        }
        "riscv64gc-unknown-none-elf" | "riscv64imac-unknown-none-elf" => {
            println!("cargo:rerun-if-changed=linker/riscv64.ld");
            "linker/riscv64.ld"
        }
        "loongarch64-unknown-none" => {
            println!("cargo:rerun-if-changed=linker/loongarch64.ld");
            "linker/loongarch64.ld"
        }
        _ => {
            panic!("Unsupported target architecture: {}", target);
        }
    };

    // 告诉 cargo 链接脚本的位置
    println!("cargo:rustc-link-arg=-T{}", linker_script);

    // 设置架构特定的编译选项
    match target.as_str() {
        "x86_64-unknown-none" => {
            // x86_64 特定选项
            println!("cargo:rustc-link-arg=-z");
            println!("cargo:rustc-link-arg=max-page-size=0x1000");
            println!("cargo:rustc-cfg=arch_x86_64");
        }
        "aarch64-unknown-none" => {
            // AArch64 特定选项
            println!("cargo:rustc-cfg=arch_aarch64");
        }
        "riscv64gc-unknown-none-elf" | "riscv64imac-unknown-none-elf" => {
            // RISC-V 特定选项
            println!("cargo:rustc-cfg=arch_riscv64");
        }
        "loongarch64-unknown-none" => {
            // LoongArch 特定选项
            println!("cargo:rustc-cfg=arch_loongarch64");
        }
        _ => {}
    }

    // 禁用标准库
    println!("cargo:rustc-env=RUST_TARGET_PATH={}", out_dir.display());

    // 生成版本信息
    generate_version_info(&out_dir);

    // 验证链接脚本存在
    let linker_path = PathBuf::from(linker_script);
    if !linker_path.exists() {
        panic!("Linker script not found: {}", linker_script);
    }

    println!("cargo:warning=Building for target: {}", target);
    println!("cargo:warning=Using linker script: {}", linker_script);
}

/// 生成版本信息文件
fn generate_version_info(out_dir: &PathBuf) {
    use std::fs::File;
    use std::io::Write;

    let version_file = out_dir.join("version.rs");
    let mut file = File::create(version_file).unwrap();

    // 获取构建信息
    let version = env::var("CARGO_PKG_VERSION").unwrap_or_else(|_| "unknown".to_string());
    let profile = env::var("PROFILE").unwrap_or_else(|_| "unknown".to_string());
    let target = env::var("TARGET").unwrap_or_else(|_| "unknown".to_string());

    // Git 信息（如果可用）
    let git_hash = std::process::Command::new("git")
        .args(&["rev-parse", "--short", "HEAD"])
        .output()
        .ok()
        .and_then(|output| String::from_utf8(output.stdout).ok())
        .map(|s| s.trim().to_string())
        .unwrap_or_else(|| "unknown".to_string());

    let build_time = chrono::Local::now().format("%Y-%m-%d %H:%M:%S").to_string();

    // 写入版本信息
    writeln!(file, "// Auto-generated build information").unwrap();
    writeln!(file, "pub const VERSION: &str = \"{}\";", version).unwrap();
    writeln!(file, "pub const PROFILE: &str = \"{}\";", profile).unwrap();
    writeln!(file, "pub const TARGET: &str = \"{}\";", target).unwrap();
    writeln!(file, "pub const GIT_HASH: &str = \"{}\";", git_hash).unwrap();
    writeln!(file, "pub const BUILD_TIME: &str = \"{}\";", build_time).unwrap();

    println!("cargo:rerun-if-changed=.git/HEAD");
}
