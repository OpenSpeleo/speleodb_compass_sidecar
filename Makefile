.PHONY: clean test test-rust test-rust-verbose test-tauri test-common test-ui test-ui-ci lint lint-fmt lint-clippy lint-ui dev build-tauri build-ui setup update pre-commit

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
	rm -rf app/dist/
	rm -rf app/test-results/ app/playwright-report/

# ============================================================================ #
# LINTING COMMANDS
# ============================================================================ #

pre-commit:
	@command -v prek >/dev/null 2>&1 || { \
		echo "error: 'prek' is not on PATH."; \
		echo "       Local devs: run 'make setup' once to install it."; \
		echo "       CI: provision prek before running make pre-commit."; \
		exit 127; \
	}
	prek run -a

# Run all lint checks (formatting + clippy + frontend checks)
lint: lint-fmt lint-clippy lint-ui

# Check formatting
lint-fmt:
	cargo fmt --all -- --check

# Run clippy with warnings denied across the whole workspace
lint-clippy:
	cargo clippy --workspace --all-targets --all-features -- -D warnings

# Check frontend types, lint rules, and formatting
lint-ui:
	cd app && bun run typecheck
	cd app && bun run lint
	cd app && bun run format:check

# ============================================================================ #
# TEST COMMANDS
# ============================================================================ #

# Default: Test EVERYTHING (Rust + React UI), running each suite once
test: test-rust test-ui

# Run standard Rust tests (backend + common crate + tooling)
test-rust:
	cargo test --workspace

# Run Rust tests with verbose output
test-rust-verbose:
	cargo test --workspace -- --nocapture

# Run only Tauri backend tests
test-tauri:
	cargo test -p speleodb-compass-sidecar --lib

# Run only common crate tests
test-common:
	cargo test -p common

# Run React UI tests (unit/component tests + Chromium/WebKit browser checks)
test-ui:
	cd app && bun run test:ui

# Run the same UI suite in CI; missing tools fail instead of skipping tests
test-ui-ci: test-ui

# ============================================================================ #
# BUILD COMMANDS
# ============================================================================ #

# Build Tauri app
build-tauri:
	cd app && bun run tauri build

# Build UI for distribution
build-ui:
	cd app && bun run build:release

# ============================================================================ #
# DEV COMMANDS
# ============================================================================ #

# Install frontend dependencies, browsers, and Rust development tools
setup:
	# -------------------------- bun/js/ts -------------------------- #
	@command -v bun >/dev/null 2>&1 || { echo "Install Bun $$(cat app/.bun-version) first; see DEV.md."; exit 127; }
	cd app && bun install --frozen-lockfile
	cd app && PLAYWRIGHT_SKIP_BROWSER_GC=1 bunx --no-install playwright install chromium webkit
	# -------------------------- cargo-binstall -------------------------- #
	curl -L --proto '=https' --tlsv1.2 -sSf https://raw.githubusercontent.com/cargo-bins/cargo-binstall/main/install-from-binstall-release.sh | bash
	# cargo install cargo-binstall --locked
	cargo binstall --locked --no-confirm prek
	cargo binstall --locked --no-confirm cargo-outdated
	cargo binstall --locked --no-confirm cargo-machete
	cargo binstall --locked --no-confirm cargo-deny
	cargo binstall --locked --no-confirm cargo-audit
	cargo binstall --locked --no-confirm cargo-wizard
	cargo binstall --locked --no-confirm cargo-nextest
	cargo binstall --locked --no-confirm kache

dev:
	cd app && bun run tauri dev

# All Rust crates share this workspace and Cargo.lock; one update covers them.
update:
	cargo update --manifest-path "Cargo.toml"
	cargo outdated --depth 1
	cd app && bun outdated
