#!/bin/bash
# scripts/build-all.sh
# 构建所有支持的架构版本

set -e  # 遇到错误立即退出

# 颜色定义
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m' # No Color

# 获取脚本所在目录
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_ROOT="$(dirname "$SCRIPT_DIR")"

# 支持的架构列表
ARCHS="x86_64 aarch64 riscv64"

# 构建统计
TOTAL_BUILDS=0
SUCCESSFUL_BUILDS=()
FAILED_BUILDS=()
BUILD_TIMES=()

# 打印带颜色的消息
print_header() {
    echo -e "${BLUE}=========================================="
    echo -e "  $1"
    echo -e "==========================================${NC}"
}

print_success() {
    echo -e "${GREEN}✓ $1${NC}"
}

print_error() {
    echo -e "${RED}✗ $1${NC}"
}

print_warning() {
    echo -e "${YELLOW}⚠ $1${NC}"
}

print_info() {
    echo -e "${BLUE}ℹ $1${NC}"
}

# 检查必要工具
check_dependencies() {
    print_info "Checking dependencies..."

    if ! command -v cargo &> /dev/null; then
        print_error "Cargo not found!"
        echo "Please install Rust: https://rustup.rs/"
        exit 1
    fi

    if ! rustup toolchain list | grep -q nightly; then
        print_warning "Nightly toolchain not found, installing..."
        rustup install nightly
    fi

    print_success "Dependencies OK"
    echo ""
}

# 构建单个架构
build_arch() {
    local ARCH=$1
    local START_TIME=$(date +%s)

    print_header "Building: $ARCH"

    # 确定目标三元组
    case $ARCH in
        x86_64)
            TARGET="x86_64-unknown-none"
            ;;
        aarch64)
            TARGET="aarch64-unknown-none"
            ;;
        riscv64)
            TARGET="riscv64gc-unknown-none-elf"
            ;;
        loongarch64)
            TARGET="loongarch64-unknown-none"
            ;;
        *)
            print_error "Unknown architecture: $ARCH"
            return 1
            ;;
    esac

    print_info "Target: $TARGET"

    # 确保目标已安装
    if ! rustup target list --installed --toolchain nightly | grep -q "$TARGET"; then
        print_info "Adding target $TARGET..."
        rustup target add "$TARGET" --toolchain nightly || {
            print_warning "Could not add target $TARGET (may not be supported)"
            return 1
        }
    fi

    # 执行构建
    cd "$PROJECT_ROOT"

    print_info "Compiling..."
    if cargo +nightly build --release --target "$TARGET" 2>&1 | tee "build-$ARCH.log"; then
        local KERNEL_PATH="target/$TARGET/release/exokernel"

        if [ -f "$KERNEL_PATH" ]; then
            local SIZE=$(du -h "$KERNEL_PATH" | cut -f1)
            local END_TIME=$(date +%s)
            local BUILD_TIME=$((END_TIME - START_TIME))

            print_success "Build succeeded"
            print_info "Kernel: $KERNEL_PATH"
            print_info "Size: $SIZE"
            print_info "Time: ${BUILD_TIME}s"

            # 创建符号链接
            ln -sf "$KERNEL_PATH" "exokernel-$ARCH"

            SUCCESSFUL_BUILDS+=("$ARCH")
            BUILD_TIMES+=("$ARCH:${BUILD_TIME}s")

            echo ""
            return 0
        else
            print_error "Kernel file not found after build"
            FAILED_BUILDS+=("$ARCH")
            echo ""
            return 1
        fi
    else
        print_error "Compilation failed"
        FAILED_BUILDS+=("$ARCH")
        echo ""
        return 1
    fi
}

# 显示构建摘要
show_summary() {
    local END_TIME=$(date +%s)
    local TOTAL_TIME=$((END_TIME - GLOBAL_START_TIME))

    print_header "Build Summary"

    echo "Total time: ${TOTAL_TIME}s"
    echo "Total builds: $TOTAL_BUILDS"
    echo ""

    if [ ${#SUCCESSFUL_BUILDS[@]} -gt 0 ]; then
        print_success "Successful builds: ${#SUCCESSFUL_BUILDS[@]}"
        for ARCH in "${SUCCESSFUL_BUILDS[@]}"; do
            echo "  ✓ $ARCH"
            # 显示构建时间
            for TIME_INFO in "${BUILD_TIMES[@]}"; do
                if [[ $TIME_INFO == $ARCH:* ]]; then
                    echo "      Time: ${TIME_INFO#*:}"
                fi
            done
        done
        echo ""

        # 显示生成的文件
        print_info "Generated kernels:"
        for ARCH in "${SUCCESSFUL_BUILDS[@]}"; do
            if [ -L "exokernel-$ARCH" ]; then
                echo "  → exokernel-$ARCH"
            fi
        done
        echo ""
    fi

    if [ ${#FAILED_BUILDS[@]} -gt 0 ]; then
        print_error "Failed builds: ${#FAILED_BUILDS[@]}"
        for ARCH in "${FAILED_BUILDS[@]}"; do
            echo "  ✗ $ARCH (see build-$ARCH.log for details)"
        done
        echo ""
    fi

    # 询问是否创建ISO
    if [ ${#SUCCESSFUL_BUILDS[@]} -gt 0 ]; then
        echo ""
        echo "Create bootable ISO images? (y/N)"
        read -r -t 10 -n 1 CREATE_ISO || CREATE_ISO="n"
        echo ""

        if [[ $CREATE_ISO =~ ^[Yy]$ ]]; then
            for ARCH in "${SUCCESSFUL_BUILDS[@]}"; do
                if [ -f "$SCRIPT_DIR/make-grub-iso.sh" ]; then
                    print_info "Creating ISO for $ARCH..."
                    bash "$SCRIPT_DIR/make-grub-iso.sh" "$ARCH" || true
                else
                    print_warning "make-grub-iso.sh not found"
                fi
            done
        fi
    fi

    echo ""
    print_header "Complete!"
}

# 主函数
main() {
    GLOBAL_START_TIME=$(date +%s)

    print_header "Exokernel Multi-Architecture Builder"
    echo ""
    echo "Project: $PROJECT_ROOT"
    echo "Rust version: $(rustc --version)"
    echo "Architectures: $ARCHS"
    echo ""

    check_dependencies

    # 构建每个架构
    for ARCH in $ARCHS; do
        TOTAL_BUILDS=$((TOTAL_BUILDS + 1))
        build_arch "$ARCH" || true
    done

    show_summary

    # 设置退出码
    if [ ${#FAILED_BUILDS[@]} -gt 0 ]; then
        exit 1
    else
        exit 0
    fi
}

# 处理中断
trap 'echo ""; print_warning "Build interrupted!"; exit 130' INT TERM

# 执行主函数
main "$@"

