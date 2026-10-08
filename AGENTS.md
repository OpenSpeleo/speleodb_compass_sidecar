# AGENTS.md

## Git actions require an explicit user request

NEVER stage, unstage, stash, unstash (including stash apply or pop), commit, or
push unless the user explicitly asks for that specific action. Permission for
one action does not authorize any of the others. Requests to review, fix,
implement, test, or finish work do not authorize these Git actions.

Invoking a skill, plugin, workflow, or sub-agent does NOT authorize these Git
actions, even if its instructions say to perform them. This restriction also
applies to sub-agents, scripts, tools, hooks, and other indirect execution.

Preserve the existing staging area and stash entries. Never automatically stash
or unstage work to run checks. Do not reset, restore, discard, or clean user
changes without an explicit request for that action. If the user tells you to
stop Git operations, stop immediately and do not attempt to undo previous Git
actions without a new explicit request.

Guidance for AI/code agents working in the SpeleoDB Compass Sidecar repository.

This file is intentionally opinionated and feature-focused so agents can make
correct changes without re-discovering architecture every session.

## Project Overview

SpeleoDB Compass Sidecar is a Tauri v2 + React/TypeScript desktop application
that bridges SpeleoDB (cave survey database) with Compass (desktop cave
surveying software). It manages project synchronization, authentication, and
launches Compass for editing.

## Temporary agent files

Keep agent plans, task lists, TODO tracking, progress notes, review notes, and
scratch lessons outside the repository tree, including all submodules. Use a
unique task directory under `/tmp/` (for example, create one with
`mktemp -d /tmp/sdb-compass-sidecar-task.XXXXXX`) or another OS temporary
directory whose resolved path is outside every checkout.

Never create or update these working files inside the checkout, even in ignored
directories such as `tasks/`, `todos/`, or `plans/`. Never stage or commit them.
Existing tracked task and lesson files are historical references; do not append
new work to them. Keep durable product and architecture documentation in
`docs/`, without embedding task checklists or linking to temporary files. Before
an authorized commit, inspect the staged filenames and exclude all agent working
files.

## Core Principles

- **Simplicity First**: Make every change as simple as possible. Impact minimal
  code.
- **No Laziness**: Find root causes. No temporary fixes. Principal Engineer
  standards
- **Minimat Impact**: Changes should only touch what's necessary. Avoid
  introducing bugs or changing unrelated parts of the code.
- **Readability & Maintainability**: Preserve product behavior while improving
  maintainability.
- **Performance Conscious**: Be aware of the performance impact of your changes
  and try to minimize the impact on performance.
- **Refactor as necessary**: Prefer centralized logic over duplicated code,
  conditionals or per-call custom checks.
- **Tests are cheap**: Every behavior should be tested. Untested code is broken
  code.

## Preserve comments and file structure

Preserve existing explanatory comments, section headings, and visual dividers in
every file, including source code, Makefiles, configuration, and workflows. Do
not remove them as part of refactoring, migration, formatting, or cleanup while
they still describe relevant code or behavior. Update their wording when the
implementation changes. Remove them only when the corresponding code or behavior
is removed or the comment is otherwise no longer relevant. Preserve the
surrounding section structure when changing commands or implementation.

## Temporary agent files

Keep agent plans, task lists, TODO tracking, progress notes, review notes, and
scratch lessons outside the repository tree, including all submodules. Use a
unique task directory under `/tmp/` (for example, create one with
`mktemp -d /tmp/speleodb-task.XXXXXX`) or another OS temporary directory whose
resolved path is outside every checkout.

Never create or update these working files inside the checkout, even in ignored
directories such as `tasks/`, `todos/`, or `plans/`. Never stage or commit them.
Existing tracked task and lesson files are historical references; do not append
new work to them. Keep durable product and architecture documentation in
`docs/`, without embedding task checklists or linking to temporary files. Before
an authorized commit, inspect the staged filenames and exclude all agent working
files.

## Task Management

1. **Plan First**: Write `plan.md` in the external temporary task directory with
   checkable items
2. **Verify Plan**: Check in before starting implementation
3. **Track Progress**: Mark items complete as you go
4. **Explain Changes**: High-level summary at each step
5. **Document Results**: Add a review section to that temporary `plan.md`
6. **Capture Lessons**: Update `lessons.md` in the external temporary task
   directory after corrections
7. **Documentation is Key**: Document each feature and design inside `docs/`.
   What is the feature being implemented, the design space and intents and a
   rapid summary of the approach taken with key APIs & concepts.

## Workflow Orchestration

### 1. Plan Node Default

- Enter plan mode for ANY non-trivial task (3+ steps or architectural decisions)
- If something goes sideways, STOP and re-plan immediately - don't keep pushing
- Use plan mode for verification steps, not just building
- Write detailed specs upfront to reduce ambiguity

### 2. Subagent Strategy

- Use subagents liberally to keep main context window clean
- Offload research, exploration, and parallel analysis to subagents
- For complex problems, throw more compute at it via subagents
- One tack per subagent for focused execution

### 3. Self-Improvement Loop

- After ANY correction from the user: record the pattern in `lessons.md` in the
  external temporary task directory
- Write rules for yourself that prevent the same mistake
- Ruthlessly iterate on these lessons until mistake rate drops
- Review lessons at session start for relevant project

### 4. Verification Before Done

- Never mark a task complete without proving it works
- Diff behavior between master and your changes when relevant
- Ask yourself: "Would a staff engineer approve this?"
- Run tests, check logs, demonstrate correctness

### 5. Demand Elegance (Balanced)

- For non-trivial changes: pause and ask "is there a more elegant way?"
- If a fix feels hacky: "Knowing everything I know now, implement the elegant
  solution"
- Skip this for simple, obvious fixes - don't over-engineer
- Challenge your own work before presenting it

### 6. Autonomous Bug Fizing

- When given a bug report: just fix it. Don't ask for hand-holding
- Point at logs, errors, failing tests - then resolve them
- Zero context switching required from the user
- Go fix failing CI tests without being told how

## Testing Requirements

Run `make test-ui` for frontend changes (Vitest and Playwright) and relevant
native Cargo tests for backend/shared contracts. Run `make lint` for both Rust
and frontend checks. The API integration suite uses real SpeleoDB credentials;
unit tests and browser presentation fixtures do not claim native/API coverage.
See `TESTING.md` for browser setup, native smoke tests and credential policy.

## Linter

Run `make lint` to validate the codebase. This depends on:

- `make lint-fmt` — `cargo fmt --all -- --check`
- `make lint-ui` — TypeScript, ESLint and frontend formatting checks
- `make lint-clippy` —
  `cargo clippy --workspace --all-targets --all-features -- -D warnings`

Clippy runs with `-D warnings` so any clippy lint blocks the lint stage and CI.
Fix the lint at its root rather than `#[allow(...)]`-annotating it unless there
is a documented reason.

## Documentation Expectations for Agents

When changing feature behavior or architecture, update docs under `docs/` for
the impacted topic:

- feature intent
- engineering scope and ownership boundaries
- testing and verification strategy
- performance implications

Do not only document "what changed"; include "why this architecture exists".

## Changelog Maintenance

Keep the `## [Unreleased]` section of `CHANGELOG.md` current throughout
development. Every change worth mentioning in the next release—including
user-facing behavior, bug fixes, dependencies, developer tooling, CI/release
work, and significant documentation changes—must add or update a concise bullet
under the appropriate heading in the same change. Do not defer changelog updates
until release preparation.

When cutting a release, move the accumulated entries into the new version
section and restore an empty `## [Unreleased]` section for subsequent work.
Preserve previously released sections except when correcting an historical
error.

## Performance and Regression Checklist

1. Keep Rust authoritative for permissions, files, project locks and polling.
2. Resolve import selection from the preview graph without file rescans.
3. Preserve exact UI copy, layout, CSS, SVGs, focus and scroll behavior.
4. Keep one cleaned-up IPC subscription; background snapshots must not reset
   forms.
5. Run native and browser checks; browser screenshots do not prove native
   parity.

## Practical Do/Do-Not

### Do:

- Prefer shared utilities/modules over code duplication.
- Add focused tests when changing anything of significance.
- Be performance conscious.
- Systematically document all features & architectural decisions.

### Do not:

- Duplicate code or logic.
- Introduce "quick patches" that hinder long term maintainability.
- Add expensive computations.

## Build and test commands

```bash
make setup          # Bun frozen install, Playwright browsers, Rust helpers
make dev            # Tauri + Vite on fixed port 1420
make build-tauri    # packaged release
make build-ui       # production frontend
make lint           # Rust + TypeScript/frontend checks
make test           # native suite once, frontend unit + browser suites
make test-rust      # cargo test --workspace
make test-ui        # bun run test:ui from app/
make test-tauri     # Tauri backend library tests
make test-common    # shared Rust contracts
```

Use the Bun version in `app/.bun-version` for packages and frontend tools. Keep
`[run] bun = true` in `app/bunfig.toml` so executable children also use Bun.
Browser installation uses `bunx --bun --no-install`; the Markdown formatter hook
uses prek's Bun language with the system Bun binary. Do not add Node
installation steps or npm/Yarn/pnpm lockfiles. Retain Vitest through
`bun run test`, not Bun's separate test runner. `make test-ui` fails if browser
tooling is missing. Real HTTP tests load `.env`; valid `TEST_SPELEODB_INSTANCE`
and `TEST_SPELEODB_OAUTH` are required for API coverage. Deterministic
unit/browser tests use fixtures without contacting SpeleoDB.

## Workspace Structure

Four native Cargo members (resolver v3, Rust edition 2024):

- **api/** — SpeleoDB REST client and HTTP policy.
- **app/src-tauri/** — native commands, state, Compass integration and OS
  actions.
- **common/** — serialized types, shared errors and native domain helpers.
- **xtask/** — versioning and workflow regression tests.

**app/** is a separate private, unversioned React/TypeScript package. Vite emits
`app/dist`; `app/src/lib` owns IPC/types and pure frontend helpers;
`app/src/components` owns presentation and interactions. Rust handles all server
and filesystem operations. See `docs/react-frontend.md` for boundaries.

## Architecture

### Data Flow

1. **Authentication**: Frontend calls `auth_request` command → backend calls api
   crate → credentials stored in `~/.compass/user_prefs.json` (TOML format,
   0o600 permissions on Unix)

2. **Project Sync**: Backend fetches from SpeleoDB API → compares with local
   `.revision.txt` files → emits LocalProjectStatus (RemoteOnly, EmptyLocal,
   UpToDate, OutOfDate, Dirty, DirtyAndOutOfDate)

3. **Compass Launch**: Backend acquires project mutex via API → launches
   wcomp32.exe (Windows) → monitors process via sysinfo crate → releases mutex
   when Compass closes. On macOS/Linux, opens the project folder in the system
   file explorer instead.

4. **Background Tasks**: `AppState` runs a background async task polling every
   120s for remote project updates and every 1s for local status changes
   (including Compass process monitoring on Windows).

### Key Files

**Backend (app/src-tauri/src/)**

- `lib.rs` - Tauri app setup: plugins (updater, dialog), command registration,
  window close prevention if Compass is open, native menu event dispatch, Sentry
  init
- `macos_menu.rs` - macOS-only cleanup for operating-system menu items injected
  alongside the application-owned native actions
- `commands.rs` - Tauri commands: `about_info`, `auth_request`,
  `clear_active_project`, `create_project`, `discard_changes`,
  `ensure_initialized`, `open_project`, `pick_compass_project_file`,
  `preview_compass_import`, `confirm_compass_import`, `cancel_compass_import`,
  `reimport_compass_project`, `release_project_mutex`, `save_project`,
  `set_active_project`, `sign_out`, and update/diagnostic actions
- `state.rs` - `AppState` with Mutex-protected fields (api_info, project_info
  HashMap, active_project, compass_pid, loading_state), unified desktop menu
  layout, background task, `emit_app_state_change()` to push `UiState` to
  frontend via `UI_STATE_EVENT`
- `paths.rs` - Path constants and helpers: `~/.compass/` home dir,
  `~/.compass/projects/{uuid}/index` and `working_copy` layout, file logger
  setup
- `user_prefs.rs` - `UserPrefs` persistence: load/save TOML credentials, env var
  fallback for tests (`TEST_SPELEODB_INSTANCE`, `TEST_SPELEODB_OAUTH`)
- `project_management/mod.rs` - `ProjectManager`: local status detection,
  project download/upload, mutex management
- `project_management/local_project.rs` - `LocalProject`: Compass file handling
  (`.MAK`, `.DAT`, `.PLT`), dirty detection (index vs working_copy), ZIP
  packing, project import via `compass_data` crate
- `project_management/revision.rs` - `.revision.txt` read/write for tracking
  synced commit hash
- `initial_import.rs` and `project_management/import.rs` - preview/session
  lifecycle, source analysis, safe staging and selective Compass import
- `self_update.rs` - non-blocking update checks, progress and native
  install/restart

**Frontend (app/src/)**

- React entry/root subscribe to `ui-state-update`, initialize the existing
  backend, and select auth/loading/main presentation.
- `lib/controller.ts` wraps Tauri `invoke()` with the existing command names,
  payload keys, input validation and error presentation.
- `lib/types.ts` models actual Rust serialization; contract tests must cover
  enum representations, nullable fields and command arguments.
- `lib/import-selection.ts` mirrors the Rust selection resolver for immediate
  feedback; Rust recomputes and validates before writes.
- Components preserve authentication, project listing/details, creation/import,
  modal and updater behavior. CSS remains in `app/styles.css`.
- `app/about.html` remains a separate static entry using the global Tauri API.

**API (api/src/)**

- `lib.rs` - Module declarations, global HTTP client (`reqwest`) with 10s
  timeout
- `http.rs` - Centralized v2 plumbing: `v2_url()`, `authenticated()`,
  `send_json()`, `send_raw()`, `map_status_to_error()`. Single chokepoint for
  status-code → typed `Error` mapping (401/403→Unauthorized, 404→NotFound,
  422→Unprocessable, 409/423→Conflict, otherwise `Api{status,message}`)
- `auth.rs` - `authorize_with_token()` and `authorize_with_email()` against
  `api/v2/user/auth-token/`
- `project.rs` - `create_project()`, `fetch_project_info()`, `fetch_projects()`,
  `acquire_project_mutex()` (Conflict→`ProjectMutexLocked`),
  `release_project_mutex()`, `download_project_zip()`
  (Unprocessable→`NoProjectData`), `upload_project_zip()` — all under
  `api/v2/projects/`
- `test_support.rs` (test-only) - `.env` autoloader, `test_api_info()`,
  `unauthorized_api_info()`, `fixture_project_id()` (lazy shared OnceCell),
  `build_minimal_compass_zip()`. See `docs/api-v2.md` for the testing strategy.

**Common (common/src/)**

- `lib.rs` - Re-exports, conditional `API_BASE_URL` (stage in debug, production
  in release)
- `api_info.rs` - `ApiInfo` (instance URL, email, oauth_token), `OauthToken`
  newtype
- `api_types.rs` - `ProjectInfo`, `CommitInfo`, `ProjectType` (Compass,
  Ignored), `ProjectSaveResult`
- `ui_state.rs` - `UiState`, `LoadingState`, `LocalProjectStatus`,
  `ProjectStatus`, `Platform`
- `error.rs` - Single `Error` enum covering auth, file I/O, project state,
  network, OS/Compass, serialization. HTTP-derived variants:
  `Unauthorized(String)`, `NotFound(String)`, `Unprocessable(String)`,
  `Conflict(String)`, generic `Api{status,message}` — every variant carries the
  server-provided message.

### IPC Communication

- Frontend → Backend: typed controller calls `invoke()` from `@tauri-apps/api`.
- Backend → Frontend: `emit()` events via `UI_STATE_EVENT` ("ui-state-update")
- Frontend registers one event subscription before initialization and cleans it
  up on WebView teardown or development store-module replacement.

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

## Windows Development Setup

The native Tauri backend still requires Rust. For local Windows development with
MinGW:

```bash
rustup toolchain install stable-x86_64-pc-windows-gnu
rustup default stable-x86_64-pc-windows-gnu
rustup component add rustfmt clippy
```

Install MSYS2 and run the following in its terminal:

```bash
pacman -S --needed base-devel mingw-w64-ucrt-x86_64-toolchain \
    mingw-w64-ucrt-x86_64-nasm
```

Add `C:\msys64\ucrt64\bin` to PATH and restart the terminal. Bun installs the
Tauri CLI with the frontend dependencies; `make setup` prepares the remaining
tools. See `DEV.md` for Rust installation and other native prerequisites.

## Key Dependencies

- **Tauri 2** with `tauri-plugin-dialog` and `tauri-plugin-updater` — native
  application shell, file dialogs and signed application updates.
- **React 19 + TypeScript** — frontend components, state and typed IPC; **Vite**
  builds the frontend and **Bun 1.4.2** manages packages and runs tools.
- **compass_data 0.0.7** — parses Compass survey file formats.
- **sentry 0.49** — native error tracking.
- **reqwest 0.13** with rustls — SpeleoDB HTTP client.
- **sysinfo 0.39** (Windows only) — monitors the Compass process.
- **zip 8** — packages project files for upload and download.

Cargo manifests and `Cargo.lock` remain authoritative for native requirements
and resolutions; `app/package.json` and `app/bun.lock` serve the frontend.
`app/.bun-version` pins Bun. Do not upgrade native dependencies as a side effect
of frontend work.

## CI/CD

- `ci.yml` runs Rust/frontend lint on Ubuntu, then native tests, browser tests
  and native builds on Windows and macOS. Trusted pushes require real API
  credentials. Secretless PRs cannot claim real-network test coverage.
- `publish.yml` uses pinned Bun, a frozen frontend install and
  `projectPath: app`. Preserve signed updater artifacts, draft releases, app
  identity, Sentry input, macOS signing and the Windows manifest. Run
  `cargo test -p xtask --test release_workflow` after workflow changes.
- `dependabot.yml` maintains Cargo, Bun and Actions independently. Cargo updates
  keep their manifest-only policy; Bun updates include its text lockfile.

See `DEV.md` for native prerequisites and `docs/release-workflow.md` for release
contracts. No WASM target, Trunk or wasm-pack is needed for this React app.

## Version Info

- Root `Cargo.toml` `[workspace.package].version` and
  `app/src-tauri/tauri.conf.json`
- `SPELEODB_COMPASS_TOML_VERSION` in `local_project.rs`: `1.0.0` schema marker
  for local `compass.toml` metadata
