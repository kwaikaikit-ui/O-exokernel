#!/bin/bash
# scripts/clean.sh
# 清理构建产物

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_ROOT="$(dirname "$SCRIPT_DIR")"

echo "Cleaning build artifacts..."

cd "$PROJECT_ROOT"

# Cargo清理
cargo clean

# 删除ISO文件
rm -f exokernel*.iso

# 删除ISO根目录
rm -rf isoroot

# 删除符号链接
rm -f exokernel-*

# 删除构建日志
rm -f build-*.log

# 删除临时文件
find . -name "*.o" -delete
find . -name "*.a" -delete
find . -name "*.so" -delete

echo "Clean complete!"


