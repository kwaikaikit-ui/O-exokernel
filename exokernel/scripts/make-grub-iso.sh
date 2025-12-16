#!/bin/bash
# scripts/make-grub-iso.sh
# 创建可启动的GRUB ISO镜像

set -e

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_ROOT="$(dirname "$SCRIPT_DIR")"

ARCH=${1:-x86_64}
ISO_NAME="exokernel-${ARCH}.iso"

echo "======================================"
echo "  Creating GRUB Bootable ISO"
echo "  Architecture: $ARCH"
echo "======================================"
echo ""

# 检查必要工具
if ! command -v grub-mkrescue &> /dev/null; then
    echo "ERROR: grub-mkrescue not found"
    echo "Install with: sudo apt install grub-pc-bin xorriso"
    exit 1
fi

# 设置目标
case $ARCH in
    x86_64)
        TARGET="x86_64-unknown-none"
        KERNEL_NAME="kernel.elf"
        ;;
    aarch64)
        TARGET="aarch64-unknown-none"
        KERNEL_NAME="kernel-aarch64.elf"
        ;;
    riscv64)
        TARGET="riscv64gc-unknown-none-elf"
        KERNEL_NAME="kernel-riscv64.elf"
        ;;
    *)
        echo "ERROR: Unsupported architecture: $ARCH"
        exit 1
        ;;
esac

KERNEL_PATH="$PROJECT_ROOT/target/$TARGET/release/exokernel"

# 检查内核是否存在
if [ ! -f "$KERNEL_PATH" ]; then
    echo "Kernel not found at $KERNEL_PATH"
    echo "Building kernel first..."
    cd "$PROJECT_ROOT"
    cargo build --release --target "$TARGET"
fi

# 创建ISO目录结构
echo "[1/5] Creating ISO directory structure..."
ISO_ROOT="$PROJECT_ROOT/isoroot"
rm -rf "$ISO_ROOT"
mkdir -p "$ISO_ROOT/boot/grub"

# 复制内核
echo "[2/5] Copying kernel..."
cp "$KERNEL_PATH" "$ISO_ROOT/boot/$KERNEL_NAME"

# 创建GRUB配置
echo "[3/5] Creating GRUB configuration..."
cat > "$ISO_ROOT/boot/grub/grub.cfg" << EOF
set timeout=3
set default=0

# 设置主题
set color_normal=white/black
set color_highlight=black/light-gray

menuentry "Exokernel ($ARCH)" {
    echo "Loading Exokernel..."
    multiboot2 /boot/$KERNEL_NAME
    boot
}

menuentry "Exokernel ($ARCH) - Debug Mode" {
    echo "Loading Exokernel in debug mode..."
    multiboot2 /boot/$KERNEL_NAME debug
    boot
}

menuentry "Exokernel ($ARCH) - Safe Mode" {
    echo "Loading Exokernel in safe mode..."
    multiboot2 /boot/$KERNEL_NAME safe
    boot
}

menuentry "Reboot" {
    reboot
}

menuentry "Shutdown" {
    halt
}
EOF

# 添加README
cat > "$ISO_ROOT/README.txt" << EOF
Exokernel - Rust Ownership-based Operating System

Architecture: $ARCH
Build Date: $(date)

This is a bootable ISO image of the Exokernel.

Features:
- Multi-architecture support (x86_64, aarch64, riscv64, loongarch64)
- Rust ownership model for resource management
- GRUB bootloader support
- Minimal external dependencies

For more information, visit: https://github.com/yourproject/exokernel
EOF

# 生成ISO
echo "[4/5] Generating ISO image..."
cd "$PROJECT_ROOT"
grub-mkrescue -o "$ISO_NAME" "$ISO_ROOT" 2>&1 | grep -v "WARNING: Couldn't find"

if [ -f "$ISO_NAME" ]; then
    ISO_SIZE=$(du -h "$ISO_NAME" | cut -f1)
    echo "[5/5] ISO created successfully!"
    echo ""
    echo "======================================"
    echo "  ISO Information"
    echo "======================================"
    echo "File: $ISO_NAME"
    echo "Size: $ISO_SIZE"
    echo "Architecture: $ARCH"
    echo ""
    echo "Test with QEMU:"
    echo "  qemu-system-$ARCH -cdrom $ISO_NAME -serial stdio -m 256M"
    echo ""
    echo "Write to USB drive:"
    echo "  sudo dd if=$ISO_NAME of=/dev/sdX bs=4M status=progress && sync"
    echo ""
    echo "Boot on real hardware:"
    echo "  1. Write to USB as shown above"
    echo "  2. Boot from USB in BIOS/UEFI"
    echo "  3. Serial output available on COM1 (115200 baud)"
    echo "======================================"
else
    echo "ERROR: Failed to create ISO"
    exit 1
fi

# 清理临时文件（可选）
# rm -rf "$ISO_ROOT"


