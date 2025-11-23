.PHONY: clean test test-rust test-rust-verbose test-tauri test-common test-ui coverage build install lint dev build-tauri build-ui

# ============================================================================ #
# CLEAN COMMANDS
# ============================================================================ #

clean:
	rm -fr dist/
	rm -fr target/

# ============================================================================ #
# TEST COMMANDS
# ============================================================================ #

# Default: Test EVERYTHING (Rust + WASM UI)
test: test-rust test-ui

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
	cargo test -p speleodb_compass_common

# Run WASM UI tests (requires wasm-pack)
test-ui:
	cd src && wasm-pack test --headless --firefox

# ============================================================================ #
# BUILD COMMANDS
# ============================================================================ #

# Build Tauri app
build-tauri:
	cargo tauri build

# Build UI for distribution
build-ui:
	trunk build --release

# ============================================================================ #
# DEV COMMANDS
# ============================================================================ #

dev:
	cargo tauri dev