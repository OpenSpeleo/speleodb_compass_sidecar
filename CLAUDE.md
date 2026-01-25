# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Project Overview

SpeleoDB Compass Sidecar is a Tauri v2 + Yew desktop application that bridges SpeleoDB (cave survey database) with Compass (desktop cave surveying software). It manages project synchronization, authentication, and launches Compass for editing.

## Build Commands

```bash
# Development with hot-reload
make dev
# or: cd app && cargo tauri dev

# Build release
make build-tauri
# or: cd app && cargo tauri build

# Build UI only (WASM)
make build-ui
# or: cd app && trunk build --release
```

## Testing

All tests make **real HTTP requests** - no mocks. Requires `.env` file with valid credentials.

```bash
# Setup: copy .env.dist to .env and add credentials
cp .env.dist .env
# Edit .env with TEST_SPELEODB_INSTANCE and TEST_SPELEODB_OAUTH

# Test everything (Rust + WASM)
make test

# Test only Rust (excludes WASM UI)
make test-rust

# Test specific crates
make test-tauri    # Tauri backend only
make test-common   # Common crate only
make test-ui       # WASM UI tests (requires wasm-pack)

# Verbose output
make test-rust-verbose

# Run specific test
cargo test native_auth_request

# Run tests serially (prevents race conditions)
cargo test -- --test-threads=1
```

## Workspace Structure

Five Cargo workspace members:

- **api/** - SpeleoDB REST API client (auth, project CRUD, mutex acquire/release)
- **app/** - Yew WASM frontend (components, IPC to backend)
- **app/src-tauri/** - Tauri backend (commands, state management, Compass integration)
- **common/** - Shared types (ApiInfo, ProjectInfo, UiState, LoadingState)
- **errors/** - Single Error enum with ~30 variants, serializable for frontend

## Architecture

### Data Flow

1. **Authentication**: Frontend calls `auth_request` command → backend calls api crate → credentials stored in `~/.speleodb_compass/api_info.toml`

2. **Project Sync**: Backend fetches from SpeleoDB API → compares with local `.revision.txt` files → emits LocalProjectStatus (RemoteOnly, UpToDate, OutOfDate, Dirty, etc.)

3. **Compass Launch**: Backend acquires project mutex via API → launches comp32.exe (Windows) → monitors process → releases mutex when Compass closes

### Key Files

**Backend (app/src-tauri/src/)**
- `commands.rs` - Tauri commands exposed to frontend
- `state.rs` - AppState with background tasks (project status updates every 120s)
- `project_management/mod.rs` - ProjectManager for local project operations

**Frontend (app/src/)**
- `speleo_db_controller.rs` - Wrapper for Tauri IPC calls
- `components/project_details.rs` - Main project view
- `components/auth_screen.rs` - Login UI

**API (api/src/)**
- `auth.rs` - OAuth and email/password authentication
- `project.rs` - Project operations including mutex handling

### IPC Communication

- Frontend → Backend: `invoke()` calls (JSON serialized)
- Backend → Frontend: `emit()` events via `UI_STATE_NOTIFICATION_KEY`

## Logging

Logs written to: `~/.speleodb_compass/speleodb_compass.log`

```bash
# Real-time logs
tail -f ~/.speleodb_compass/speleodb_compass.log

# Search logs
grep "pattern" ~/.speleodb_compass/speleodb_compass.log
```

## Windows Development Setup

Requires Windows toolchain with MinGW:

```bash
rustup toolchain install stable-x86_64-pc-windows-gnu
rustup default stable-x86_64-pc-windows-gnu
rustup target add wasm32-unknown-unknown
cargo install tauri-cli --version "^2.0.0" --locked
cargo install trunk --locked
cargo install wasm-pack
```

Also requires MSYS2 with `base-devel` and `mingw-w64-ucrt-x86_64-toolchain` packages.

## CI/CD

- `ci.yml` - Runs on push/PR, executes `make test` on Windows
- `publish.yml` - Triggered by git tags, depends on CI passing
