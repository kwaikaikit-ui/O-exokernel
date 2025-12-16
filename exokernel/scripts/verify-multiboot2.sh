#!/bin/bash
# scripts/verify-multiboot2.sh

KERNEL=$1

if [ ! -f "$KERNEL" ]; then
    echo "Usage: $0 <kernel-file>"
    exit 1
fi

echo "Checking Multiboot2 header..."

# 检查魔数
MAGIC=$(od -An -t x4 -N 4 "$KERNEL" | tr -d ' ')
if [ "$MAGIC" = "e85250d6" ]; then
    echo "✓ Multiboot2 magic found: 0x$MAGIC"
else
    echo "✗ Invalid magic: 0x$MAGIC (expected 0xe85250d6)"
    exit 1
fi

# 使用 grub-file 检查
if command -v grub-file &> /dev/null; then
    if grub-file --is-x86-multiboot2 "$KERNEL"; then
        echo "✓ Valid Multiboot2 kernel (grub-file)"
    else
        echo "✗ Invalid Multiboot2 kernel"
    fi
fi

echo "Header verification complete!"
