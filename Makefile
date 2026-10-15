# 供类 Unix 环境参考；Windows 请使用 make.ps1（功能一一对应）

.PHONY: check check-layers build release run clean fmt clippy frontend-build frontend-dev

FRONTEND_DIR := crates/frontend

check:
	cargo check --workspace

check-layers:
	pwsh -File ./scripts/check_layers.ps1

build:
	cargo build

frontend-build:
	cd $(FRONTEND_DIR) && npm install --no-audit --no-fund && npm run build

release: frontend-build
	cargo build --release

run: release
	./target/x86_64-win7-windows-msvc/release/food_accept_single.exe

frontend-dev:
	cd $(FRONTEND_DIR) && npm run dev

fmt:
	cargo fmt --all

clippy:
	cargo clippy --workspace

clean:
	cargo clean
