# Exokernel 多架构构建系统

.PHONY: all build run clean iso help test debug release install-deps

# 默认架构
ARCH ?= x86_64

# 根据架构设置目标三元组
ifeq ($(ARCH),x86_64)
    TARGET := x86_64-unknown-none
    QEMU := qemu-system-x86_64
    QEMU_ARGS := -serial stdio -m 256M -no-reboot -display none
endif

ifeq ($(ARCH),aarch64)
    TARGET := aarch64-unknown-none
    QEMU := qemu-system-aarch64
    QEMU_ARGS := -M virt -cpu cortex-a72 -serial stdio -m 256M -display none
endif

ifeq ($(ARCH),riscv64)
    TARGET := riscv64gc-unknown-none-elf
    QEMU := qemu-system-riscv64
    QEMU_ARGS := -M virt -serial stdio -m 256M -display none
endif

ifeq ($(ARCH),loongarch64)
    TARGET := loongarch64-unknown-none
    QEMU := qemu-system-loongarch64
    QEMU_ARGS := -M virt -serial stdio -m 256M -display none
endif

# 路径配置
CARGO := cargo +nightly
BUILD_MODE ?= release
KERNEL := target/$(TARGET)/$(BUILD_MODE)/exokernel
ISO_DIR := isoroot
ISO_FILE := exokernel-$(ARCH).iso

# 颜色输出
GREEN := \033[0;32m
BLUE := \033[0;34m
YELLOW := \033[1;33m
NC := \033[0m # No Color

# 默认目标
all: build

# 构建内核
build:
	@echo "$(BLUE)Building for $(ARCH)...$(NC)"
	$(CARGO) build --$(BUILD_MODE) --target $(TARGET)
	@echo "$(GREEN)✓ Build complete: $(KERNEL)$(NC)"

# 开发模式构建
dev:
	@$(MAKE) build BUILD_MODE=dev

# 发布模式构建
release:
	@$(MAKE) build BUILD_MODE=release

# 运行内核（QEMU）
run: build
	@echo "$(BLUE)Running on QEMU ($(ARCH))...$(NC)"
	@echo "$(YELLOW)Press Ctrl+A then X to exit$(NC)"
	$(QEMU) $(QEMU_ARGS) -kernel $(KERNEL)

# 运行并保存串口输出
run-log: build
	@echo "$(BLUE)Running and logging to kernel.log...$(NC)"
	$(QEMU) $(QEMU_ARGS) -kernel $(KERNEL) 2>&1 | tee kernel.log

# 调试模式运行
debug: build
	@echo "$(BLUE)Starting QEMU with GDB server on :1234...$(NC)"
	@echo "Connect with: gdb -ex 'target remote :1234' $(KERNEL)"
	$(QEMU) $(QEMU_ARGS) -kernel $(KERNEL) -s -S

# 创建ISO镜像
iso: build
	@echo "$(BLUE)Creating bootable ISO...$(NC)"
	@mkdir -p $(ISO_DIR)/boot/grub
	@cp $(KERNEL) $(ISO_DIR)/boot/kernel.elf
	@echo 'set timeout=0' > $(ISO_DIR)/boot/grub/grub.cfg
	@echo 'set default=0' >> $(ISO_DIR)/boot/grub/grub.cfg
	@echo 'menuentry "Exokernel" {' >> $(ISO_DIR)/boot/grub/grub.cfg
	@echo '    multiboot2 /boot/kernel.elf' >> $(ISO_DIR)/boot/grub/grub.cfg
	@echo '    boot' >> $(ISO_DIR)/boot/grub/grub.cfg
	@echo '}' >> $(ISO_DIR)/boot/grub/grub.cfg
	@grub-mkrescue -o $(ISO_FILE) $(ISO_DIR) 2>/dev/null || \
		echo "$(YELLOW)grub-mkrescue not found, skipping ISO creation$(NC)"
	@if [ -f $(ISO_FILE) ]; then \
		echo "$(GREEN)✓ ISO created: $(ISO_FILE)$(NC)"; \
	fi

# 测试ISO
test-iso: iso
	@echo "$(BLUE)Testing ISO in QEMU...$(NC)"
	$(QEMU) $(QEMU_ARGS) -cdrom $(ISO_FILE)

# 构建所有架构
build-all:
	@echo "$(BLUE)Building all architectures...$(NC)"
	@$(MAKE) build ARCH=x86_64
	@$(MAKE) build ARCH=aarch64
	@$(MAKE) build ARCH=riscv64
	@echo "$(GREEN)✓ All builds complete$(NC)"

# 清理构建产物
clean:
	@echo "$(BLUE)Cleaning build artifacts...$(NC)"
	$(CARGO) clean
	rm -rf $(ISO_DIR) *.iso *.log
	rm -f exokernel-* build-*.log
	@echo "$(GREEN)✓ Clean complete$(NC)"

# 深度清理（包括Cargo缓存）
clean-all: clean
	rm -rf target/
	@echo "$(GREEN)✓ Deep clean complete$(NC)"

# 代码检查
check:
	@echo "$(BLUE)Running cargo check...$(NC)"
	$(CARGO) check --target $(TARGET)

# 代码格式化
fmt:
	@echo "$(BLUE)Formatting code...$(NC)"
	$(CARGO) fmt

# Clippy检查
clippy:
	@echo "$(BLUE)Running clippy...$(NC)"
	$(CARGO) clippy --target $(TARGET) -- -D warnings

# 运行测试（如果有）
test:
	@echo "$(BLUE)Running tests...$(NC)"
	$(CARGO) test --lib

# 生成文档
doc:
	@echo "$(BLUE)Generating documentation...$(NC)"
	$(CARGO) doc --target $(TARGET) --no-deps --open

# 显示内核信息
info: build
	@echo "$(BLUE)Kernel Information:$(NC)"
	@echo "  Architecture: $(ARCH)"
	@echo "  Target: $(TARGET)"
	@echo "  Build mode: $(BUILD_MODE)"
	@echo "  Kernel path: $(KERNEL)"
	@if [ -f $(KERNEL) ]; then \
		echo "  Size: $$(du -h $(KERNEL) | cut -f1)"; \
		file $(KERNEL); \
	fi

# 反汇编内核
disasm: build
	@echo "$(BLUE)Disassembling kernel...$(NC)"
	rust-objdump -d $(KERNEL) > kernel.asm
	@echo "$(GREEN)✓ Disassembly saved to kernel.asm$(NC)"

# 查看符号表
symbols: build
	@echo "$(BLUE)Kernel symbols:$(NC)"
	rust-nm $(KERNEL) | head -50

# 安装依赖（Linux）
install-deps:
	@echo "$(BLUE)Installing dependencies...$(NC)"
	@if command -v apt-get >/dev/null 2>&1; then \
		sudo apt-get update && \
		sudo apt-get install -y qemu-system-x86 qemu-system-arm qemu-system-misc \
		                         grub-pc-bin xorriso mtools; \
	elif command -v dnf >/dev/null 2>&1; then \
		sudo dnf install -y qemu-system-x86 qemu-system-aarch64 qemu-system-riscv \
		                     grub2-tools-extra xorriso mtools; \
	elif command -v pacman >/dev/null 2>&1; then \
		sudo pacman -S --needed qemu-system-x86 qemu-system-arm qemu-system-riscv \
		                         grub xorriso mtools; \
	else \
		echo "$(YELLOW)Package manager not detected, please install manually$(NC)"; \
	fi
	@echo "$(GREEN)✓ Dependencies installed$(NC)"

# 安装Rust目标
install-targets:
	@echo "$(BLUE)Installing Rust targets...$(NC)"
	rustup target add x86_64-unknown-none
	rustup target add aarch64-unknown-none
	rustup target add riscv64gc-unknown-none-elf
	@echo "$(GREEN)✓ Targets installed$(NC)"

# 监视文件变化并自动构建
watch:
	@echo "$(BLUE)Watching for changes...$(NC)"
	@while true; do \
		inotifywait -e modify -r src/ Cargo.toml 2>/dev/null && \
		$(MAKE) build; \
	done

# 打包项目
package:
	@echo "$(BLUE)Creating project archive...$(NC)"
	@tar -czf exokernel-$$(date +%Y%m%d).tar.gz \
		--exclude='target' \
		--exclude='*.iso' \
		--exclude='isoroot' \
		--exclude='*.log' \
		.
	@echo "$(GREEN)✓ Archive created: exokernel-$$(date +%Y%m%d).tar.gz$(NC)"

# USB启动盘创建（警告：会擦除USB设备）
usb: iso
	@echo "$(YELLOW)WARNING: This will erase /dev/sdX!$(NC)"
	@echo "Available devices:"
	@lsblk -d -o NAME,SIZE,TYPE | grep disk
	@read -p "Enter device (e.g., sdb): " DEVICE && \
	sudo dd if=$(ISO_FILE) of=/dev/$$DEVICE bs=4M status=progress && sync
	@echo "$(GREEN)✓ USB drive created$(NC)"

# 帮助信息
help:
	@echo "$(BLUE)Exokernel Build System$(NC)"
	@echo ""
	@echo "Usage: make [target] [ARCH=<arch>] [BUILD_MODE=<mode>]"
	@echo ""
	@echo "$(GREEN)Build Targets:$(NC)"
	@echo "  build          - Build kernel (default)"
	@echo "  dev            - Build in debug mode"
	@echo "  release        - Build in release mode"
	@echo "  build-all      - Build all architectures"
	@echo ""
	@echo "$(GREEN)Run Targets:$(NC)"
	@echo "  run            - Run in QEMU"
	@echo "  run-log        - Run and save output to log"
	@echo "  debug          - Run with GDB server"
	@echo "  iso            - Create bootable ISO"
	@echo "  test-iso       - Test ISO in QEMU"
	@echo ""
	@echo "$(GREEN)Development:$(NC)"
	@echo "  check          - Run cargo check"
	@echo "  fmt            - Format code"
	@echo "  clippy         - Run clippy lints"
	@echo "  test           - Run tests"
	@echo "  doc            - Generate documentation"
	@echo ""
	@echo "$(GREEN)Analysis:$(NC)"
	@echo "  info           - Show kernel information"
	@echo "  disasm         - Disassemble kernel"
	@echo "  symbols        - Show symbol table"
	@echo ""
	@echo "$(GREEN)Utilities:$(NC)"
	@echo "  clean          - Clean build artifacts"
	@echo "  clean-all      - Deep clean including cache"
	@echo "  install-deps   - Install system dependencies"
	@echo "  install-targets- Install Rust targets"
	@echo "  package        - Create project archive"
	@echo "  usb            - Create USB bootable drive"
	@echo ""
	@echo "$(GREEN)Variables:$(NC)"
	@echo "  ARCH           - Target architecture (x86_64, aarch64, riscv64, loongarch64)"
	@echo "  BUILD_MODE     - Build mode (debug, release)"
	@echo ""
	@echo "$(GREEN)Examples:$(NC)"
	@echo "  make run                    # Run x86_64 in QEMU"
	@echo "  make run ARCH=aarch64       # Run aarch64 in QEMU"
	@echo "  make build ARCH=riscv64     # Build for RISC-V"
	@echo "  make iso ARCH=x86_64        # Create x86_64 ISO"

# 快速启动（一键构建并运行）
quick: build run

# 显示版本信息
version:
	@echo "Exokernel v$$(grep '^version' Cargo.toml | cut -d'"' -f2)"
	@echo "Rust: $$(rustc --version)"
	@echo "Cargo: $$(cargo --version)"
