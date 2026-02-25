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

Five Cargo workspace members (resolver v3, Rust edition 2024):

- **api/** - SpeleoDB REST API client (auth, project CRUD, mutex acquire/release)
- **app/** - Yew WASM frontend (`speleodb-compass-sidecar-ui`, components + IPC to backend)
- **app/src-tauri/** - Tauri backend (`speleodb-compass-sidecar`, commands, state management, Compass integration)
- **common/** - Shared types (ApiInfo, OauthToken, ProjectInfo, UiState, LoadingState, LocalProjectStatus)
- **errors/** - Single Error enum with ~30 variants, serializable for frontend

Workspace dependencies defined in root `Cargo.toml`: bytes, log, serde, serde_json, thiserror, tokio, toml, url, uuid.

## Architecture

### Data Flow

1. **Authentication**: Frontend calls `auth_request` command → backend calls api crate → credentials stored in `~/.compass/user_prefs.json` (TOML format, 0o600 permissions on Unix)

2. **Project Sync**: Backend fetches from SpeleoDB API → compares with local `.revision.txt` files → emits LocalProjectStatus (RemoteOnly, EmptyLocal, UpToDate, OutOfDate, Dirty, DirtyAndOutOfDate)

3. **Compass Launch**: Backend acquires project mutex via API → launches wcomp32.exe (Windows) → monitors process via sysinfo crate → releases mutex when Compass closes. On macOS/Linux, opens the project folder in the system file explorer instead.

4. **Background Tasks**: `AppState` runs a background async task polling every 120s for remote project updates and every 1s for local status changes (including Compass process monitoring on Windows).

### Key Files

**Backend (app/src-tauri/src/)**
- `lib.rs` - Tauri app setup: plugins (updater, dialog), command registration, window close prevention if Compass is open, menu with sign-out, Sentry init
- `commands.rs` - Tauri commands: `about_info`, `auth_request`, `clear_active_project`, `create_project`, `discard_changes`, `ensure_initialized`, `import_compass_project`, `open_project`, `pick_compass_project_file`, `reimport_compass_project`, `release_project_mutex`, `save_project`, `set_active_project`, `sign_out`
- `state.rs` - `AppState` with Mutex-protected fields (api_info, project_info HashMap, active_project, compass_pid, loading_state), background task, `emit_app_state_change()` to push `UiState` to frontend via `UI_STATE_EVENT`
- `paths.rs` - Path constants and helpers: `~/.compass/` home dir, `~/.compass/projects/{uuid}/index` and `working_copy` layout, file logger setup
- `user_prefs.rs` - `UserPrefs` persistence: load/save TOML credentials, env var fallback for tests (`TEST_SPELEODB_INSTANCE`, `TEST_SPELEODB_OAUTH`)
- `project_management/mod.rs` - `ProjectManager`: local status detection, project download/upload, mutex management
- `project_management/local_project.rs` - `LocalProject`: Compass file handling (`.MAK`, `.DAT`, `.PLT`), dirty detection (index vs working_copy), ZIP packing, project import via `compass_data` crate
- `project_management/revision.rs` - `.revision.txt` read/write for tracking synced commit hash

**Frontend (app/src/)**
- `main.rs` - WASM entry: panic hook, wasm_logger, renders `App`
- `app.rs` - Root component: subscribes to `UI_STATE_EVENT`, calls `ensure_initialized()`, routes to AuthScreen / MainLayout / LoadingScreen based on `LoadingState`
- `speleo_db_controller.rs` - `SpeleoDBController` singleton wrapping Tauri `invoke()` calls with input validation (OAuth = 40 hex chars)
- `error.rs` - Frontend `Error` enum (Command, Serde variants)
- `ui_constants.rs` - Color palette constants (warn, alarm, good, blue, grey)
- `components/mod.rs` - Module declarations for all components
- `components/auth_screen.rs` - Login UI with OAuth token and email/password tabs, instance URL dropdown (stage/production)
- `components/main_layout.rs` - Two-pane authenticated layout: project listing + project details, header with user email and sign-out
- `components/project_listing.rs` - Scrollable project list from `UiState.project_status`
- `components/project_listing_item.rs` - Individual project row with status indicator
- `components/project_details.rs` - Project detail view: open in Compass, download, commit form, read-only indicator, mutex status
- `components/create_project_modal.rs` - New project form (name, description, country, coordinates)
- `components/loading_screen.rs` - Loading state display with status text
- `components/modal.rs` - Generic modal component (Success, Error, Info, Warning, Confirmation types)

**API (api/src/)**
- `lib.rs` - Module declarations, global HTTP client (`reqwest`) with 10s timeout
- `auth.rs` - `authorize_with_token()` and `authorize_with_email()` against SpeleoDB API
- `project.rs` - `create_project()`, `fetch_project_info()`, `acquire_project_mutex()`, `release_project_mutex()`

**Common (common/src/)**
- `lib.rs` - Re-exports, conditional `API_BASE_URL` (stage in debug, production in release)
- `api_info.rs` - `ApiInfo` (instance URL, email, oauth_token), `OauthToken` newtype
- `api_types.rs` - `ProjectInfo`, `CommitInfo`, `ProjectType` (ARIANE, COMPASS), `ProjectSaveResult`
- `ui_state.rs` - `UiState`, `LoadingState`, `LocalProjectStatus`, `ProjectStatus`, `Platform`

**Errors (errors/src/)**
- `lib.rs` - ~30 error variants covering auth, file I/O, project state, network, OS/Compass, serialization

### IPC Communication

- Frontend → Backend: `invoke()` calls via `tauri-sys` (JSON serialized with `serde-wasm-bindgen`)
- Backend → Frontend: `emit()` events via `UI_STATE_EVENT` ("ui-state-update")
- Frontend listens with Yew stream subscription in `App` component

### Local Project Layout

```
~/.compass/
├── user_prefs.json          # Credentials (TOML format despite .json extension)
├── speleodb_compass*.log    # Application logs (flexi_logger)
└── projects/
    └── {project-uuid}/
        ├── index/           # Last synced remote copy
        │   ├── .compass/    # compass.toml with SpeleoDb metadata
        │   └── ...          # Compass project files
        ├── working_copy/    # User's editable copy
        │   └── ...          # Compass project files (.MAK, .DAT, .PLT)
        └── .revision.txt    # Commit hash of last sync
```

## Logging

Logs written to: `~/.compass/speleodb_compass*.log`

```bash
# Real-time logs
tail -f ~/.compass/speleodb_compass*.log

# Search logs
grep "pattern" ~/.compass/speleodb_compass*.log
```

## Key Dependencies

- **Tauri 2** with plugins: `tauri-plugin-dialog`, `tauri-plugin-updater`
- **Yew 0.22** (CSR mode) with `yew_icons` (FontAwesome)
- **compass_data 0.0.7** - Parses Compass survey file formats
- **sentry 0.46** - Error tracking
- **reqwest 0.12** (rustls-tls) - HTTP client for SpeleoDB API
- **sysinfo 0.33** (Windows only) - Compass process monitoring
- **zip 7** - Project packaging for upload/download

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

- `ci.yml` - Runs on push/PR to main, executes `make test` on Windows. Uses cargo-binstall for trunk/wasm-pack.
- `publish.yml` - Triggered by git tags, depends on CI passing. Builds for macOS (aarch64) and Windows via `tauri-action`. Creates signed GitHub release with updater artifacts.
- `dependabot.yml` - Automated dependency updates

## Version Info

- Cargo.toml and tauri.conf.json: `0.1.0`
- `SPELEODB_COMPASS_VERSION` constant in `lib.rs`: `1.0.0`
