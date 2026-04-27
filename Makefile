.PHONY: clean test test-rust test-rust-verbose test-tauri test-common test-ui lint dev build-tauri build-ui setup

# ============================================================================ #
# CLEAN COMMANDS
# ============================================================================ #

clean:
	rm -fr dist/
	rm -fr target/

# ============================================================================ #
# TEST COMMANDS
# ============================================================================ #

# Default: Test EVERYTHING (lint + Rust + WASM UI)
test: lint test-rust test-ui test-tauri

# Check formatting
lint:
	cargo fmt --all -- --check

# Run standard Rust tests (backend + common crate)
test-rust:
	cargo test --workspace --exclude speleodb-compass-sidecar-ui

# Run Rust tests with verbose output
test-rust-verbose:
	cargo test --workspace --exclude speleodb-compass-sidecar-ui -- --nocapture

# Run only Tauri backend tests
test-tauri:
	cargo test -p speleodb-compass-sidecar --lib

# Run only common crate tests
test-common:
	cargo test -p common

# Run WASM UI tests (requires wasm-pack)
test-ui:
	@if command -v wasm-pack >/dev/null 2>&1; then \
		cd app/src && wasm-pack test --headless --firefox; \
	else \
		echo "wasm-pack not found, skipping UI tests"; \
	fi

# ============================================================================ #
# BUILD COMMANDS
# ============================================================================ #

# Build Tauri app
build-tauri:
	cd app && \
	cargo tauri build

# Build UI for distribution
build-ui:
	cd app && \
	trunk build --release

# ============================================================================ #
# DEV COMMANDS
# ============================================================================ #

# Install dev tools and set up git hooks
setup:
	cargo install --locked cargo-binstall
	cargo binstall --locked prek

dev:
	cd app && \
	cargo tauri dev
