#!/bin/bash
# scripts/install-deps.sh
# 安装所有必要的依赖

echo "=========================================="
echo "  Installing Exokernel Dependencies"
echo "=========================================="
echo ""

# 检测操作系统
if [ -f /etc/os-release ]; then
    . /etc/os-release
    OS=$ID
else
    echo "Cannot detect OS"
    exit 1
fi

echo "Detected OS: $OS"
echo ""

# 根据OS安装依赖
case $OS in
    ubuntu|debian)
        echo "Installing packages for Ubuntu/Debian..."
        sudo apt update
        sudo apt install -y \
            build-essential \
            qemu-system-x86 \
            qemu-system-arm \
            qemu-system-misc \
            grub-pc-bin \
            xorriso \
            mtools
        ;;
    fedora|rhel|centos)
        echo "Installing packages for Fedora/RHEL/CentOS..."
        sudo dnf install -y \
            gcc \
            qemu-system-x86 \
            qemu-system-aarch64 \
            qemu-system-riscv \
            grub2-tools-extra \
            xorriso \
            mtools
        ;;
    arch)
        echo "Installing packages for Arch Linux..."
        sudo pacman -S --needed \
            base-devel \
            qemu-system-x86 \
            qemu-system-arm \
            qemu-system-riscv \
            grub \
            xorriso \
            mtools
        ;;
    *)
        echo "Unsupported OS: $OS"
        echo "Please install manually:"
        echo "  - QEMU (x86, ARM, RISC-V)"
        echo "  - GRUB tools"
        echo "  - xorriso"
        exit 1
        ;;
esac

# 安装Rust（如果未安装）
if ! command -v cargo &> /dev/null; then
    echo ""
    echo "Installing Rust..."
    curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y
    source "$HOME/.cargo/env"
fi

# 安装nightly工具链
echo ""
echo "Installing Rust nightly toolchain..."
rustup install nightly
rustup component add rust-src --toolchain nightly
rustup component add llvm-tools-preview --toolchain nightly

# 添加目标
echo ""
echo "Adding target architectures..."
rustup target add x86_64-unknown-none --toolchain nightly
rustup target add aarch64-unknown-none --toolchain nightly
rustup target add riscv64gc-unknown-none-elf --toolchain nightly

echo ""
echo "=========================================="
echo "  Installation Complete!"
echo "=========================================="
echo ""
echo "Next steps:"
echo "  1. cd to project directory"
echo "  2. Run: make build"
echo "  3. Run: make run"
echo ""

