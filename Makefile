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

dev:
	cd app && \
	cargo tauri dev
