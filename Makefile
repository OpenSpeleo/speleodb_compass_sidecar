.PHONY: clean test test-rust test-rust-verbose test-tauri test-common test-ui lint lint-fmt lint-clippy dev build-tauri build-ui setup

# ============================================================================ #
# Load .env file
# ============================================================================ #

ifneq (,$(wildcard .env))
    include .env
    export
endif


# ============================================================================ #
# CLEAN COMMANDS
# ============================================================================ #

clean:
	rm -fr dist/
	rm -fr target/

# ============================================================================ #
# LINTING COMMANDS
# ============================================================================ #

# Run all lint checks (formatting + clippy)
lint: lint-fmt lint-clippy pre-commit

pre-commit:
	@command -v prek >/dev/null 2>&1 || { \
		echo "error: 'prek' is not on PATH."; \
		echo "       Local devs: run 'make setup' once to install it."; \
		echo "       CI: install via the 'Install prek' workflow step."; \
		exit 127; \
	}
	prek run -a

# Check formatting
lint-fmt:
	cargo fmt --all -- --check

# Run clippy with warnings denied across the whole workspace
lint-clippy:
	cargo clippy --workspace --all-targets --all-features -- -D warnings

# ============================================================================ #
# TEST COMMANDS
# ============================================================================ #

# Default: Test EVERYTHING (lint + Rust + WASM UI)
test: test-rust test-ui test-tauri

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
