#!/bin/bash
# scripts/test-qemu.sh
# 在QEMU中测试内核

set -e

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_ROOT="$(dirname "$SCRIPT_DIR")"

ARCH=${1:-x86_64}
MODE=${2:-normal}  # normal, debug, gdb

echo "=========================================="
echo "  Exokernel QEMU Test"
echo "  Architecture: $ARCH"
echo "  Mode: $MODE"
echo "=========================================="
echo ""

# 检查QEMU
QEMU_CMD="qemu-system-$ARCH"
if ! command -v $QEMU_CMD &> /dev/null; then
    echo "ERROR: $QEMU_CMD not found"
    echo "Install with:"
    case $ARCH in
        x86_64)
            echo "  sudo apt install qemu-system-x86"
            ;;
        aarch64)
            echo "  sudo apt install qemu-system-arm"
            ;;
        riscv64)
            echo "  sudo apt install qemu-system-misc"
            ;;
    esac
    exit 1
fi

# 设置目标和内核路径
case $ARCH in
    x86_64)
        TARGET="x86_64-unknown-none"
        QEMU_MACHINE=""
        QEMU_CPU="-cpu qemu64"
        ;;
    aarch64)
        TARGET="aarch64-unknown-none"
        QEMU_MACHINE="-M virt"
        QEMU_CPU="-cpu cortex-a72"
        ;;
    riscv64)
        TARGET="riscv64gc-unknown-none-elf"
        QEMU_MACHINE="-M virt"
        QEMU_CPU=""
        ;;
    *)
        echo "ERROR: Unsupported architecture: $ARCH"
        exit 1
        ;;
esac

KERNEL_PATH="$PROJECT_ROOT/target/$TARGET/release/exokernel"

# 检查内核
if [ ! -f "$KERNEL_PATH" ]; then
    echo "Kernel not found, building..."
    cd "$PROJECT_ROOT"
    cargo +nightly build --release --target "$TARGET"
fi

# 基础QEMU参数
QEMU_ARGS=(
    $QEMU_MACHINE
    $QEMU_CPU
    -m 256M
    -serial stdio
    -display none
    -no-reboot
    -no-shutdown
)

# 根据模式添加参数
case $MODE in
    debug)
        echo "Debug mode: verbose output enabled"
        QEMU_ARGS+=(-d int,cpu_reset)
        ;;
    gdb)
        echo "GDB mode: waiting for debugger on localhost:1234"
        echo "Connect with: gdb -ex 'target remote localhost:1234' $KERNEL_PATH"
        QEMU_ARGS+=(-s -S)
        ;;
esac

# 检查是否使用ISO
ISO_PATH="$PROJECT_ROOT/exokernel-${ARCH}.iso"
if [ -f "$ISO_PATH" ] && [ "$3" = "iso" ]; then
    echo "Booting from ISO: $ISO_PATH"
    QEMU_ARGS+=(-cdrom "$ISO_PATH")
else
    echo "Direct kernel boot: $KERNEL_PATH"
    QEMU_ARGS+=(-kernel "$KERNEL_PATH")
fi

# 显示完整命令
echo ""
echo "Running command:"
echo "$QEMU_CMD ${QEMU_ARGS[*]}"
echo ""
echo "----------------------------------------"
echo "Press Ctrl+A then X to exit QEMU"
echo "----------------------------------------"
echo ""

# 运行QEMU
exec $QEMU_CMD "${QEMU_ARGS[@]}"


