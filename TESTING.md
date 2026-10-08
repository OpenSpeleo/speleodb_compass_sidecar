# Testing

Run `make lint` and `make test` from the repository root. `make test` runs the
native workspace once, followed by frontend unit and browser tests. Missing
frontend tooling fails the run rather than silently skipping coverage.

Use the Bun version pinned in `app/.bun-version`. `app/bunfig.toml` applies the
Bun runtime to Vitest, Playwright and their executable children. Runtime
regressions verify both the Vitest worker and an inherited Node-shebang command
use that exact Bun version. `bun run test` retains the existing Vitest suite;
`bun test` is a different runner.

The [frontend contract map](docs/frontend-test-map.md) maps every former Rust UI
test to its JavaScript replacement and distinguishes browser-only obligations.

## Suites and ownership

| Command                                       | Coverage                                                           |
| --------------------------------------------- | ------------------------------------------------------------------ |
| `make lint`                                   | Rust formatting/Clippy, TypeScript, ESLint and frontend formatting |
| `make test-rust`                              | Native API, common domain, Tauri backend and xtask tests           |
| `make test-tauri`                             | Tauri backend library tests                                        |
| `make test-common`                            | Shared Rust types, serialization and import selection              |
| `cd app && bun run test`                      | Frontend pure logic and component tests                            |
| `make test-ui`                                | Frontend unit and Playwright browser tests                         |
| `make test-ui-ci`                             | Same frontend suites, retained as a CI-compatible alias            |
| `cargo test -p xtask --test release_workflow` | CI/release install, paths and packaging contracts                  |

Deterministic Rust tests use synthetic Compass files and temporary directories.
Frontend tests use explicit presentation fixtures to exercise UI states and
callbacks. Such tests prove rendering and interaction contracts, not live server
or native IPC behavior. HTTP integration tests contact a real SpeleoDB instance;
no fake API replaces that coverage.

## Real API tests

Copy `.env.dist` to `.env`, then set valid `TEST_SPELEODB_INSTANCE` and
`TEST_SPELEODB_OAUTH`. Optional email authentication tests use
`TEST_SPELEODB_EMAIL` and `TEST_SPELEODB_PASSWORD`. Never commit credentials.

```sh
cp .env.dist .env
# Edit .env with your instance URL and actual 40-character hexadecimal token.
```

The API test harness loads the workspace `.env` automatically with `dotenvy`.
Environment variables supplied by CI take precedence. `.env` remains ignored;
`.env.dist` is the committed setup template. Native user-preference tests use
`user_prefs_test.json`, separate from production `user_prefs.json`. Tests that
share mutable state retain their `#[serial]` guards.

```sh
make test-rust
cargo test -p api -- --nocapture
cargo test -p speleodb-compass-sidecar --lib project_management::import::tests
make test-rust-verbose            # native tests with output visible
cargo test native_auth_request   # one matching test
cargo test -- --test-threads=1    # serialize tests when diagnosing shared state
```

The API harness skips tests when credentials are absent. With credentials, its
shared authentication preflight reports invalid or unreachable setups before
endpoint failures cascade. Tests create permanent fixture projects because the
API has no project-delete endpoint. Use a dedicated test instance/account; mutex
lifecycle tests release acquired locks even after failures. See
[API v2](docs/api-v2.md) and [Compass import](docs/compass-import.md).

## Browser and native parity

Install dependencies with `cd app && bun install --frozen-lockfile`, then
install browser engines with
`PLAYWRIGHT_SKIP_BROWSER_GC=1 bunx --bun --no-install playwright install chromium webkit`
to preserve engine revisions used by other projects. Standard CI runs ten
browser interaction cases in each engine, covering application initialization,
authentication, project navigation/save, About, modal focus and Escape, import
selection, dependency details and scroll geometry. Unit/component tests cover
validation, loading/errors, sorting, permissions, project actions, import
recovery and updater phases.

Compare captures on the same OS, engine, viewport, scale, fonts and animation
state. Preserve the 800×900 native window, exact copy, assets, icon geometry,
focus order and scroll behavior. Investigate differences rather than accepting
new screenshots to make tests pass. Test long/Unicode text, reduced motion and
short viewports as well as default dimensions.

Native macOS and Windows checks remain required for file pickers, clipboard
roles, menus/About, updater actions, Compass/folder launch, import/save/discard,
mutex release, sign-out and shutdown. Playwright WebKit is not a substitute for
WKWebView; Chromium is not a substitute for packaged WebView2. Record any checks
not performed rather than claiming parity from browser tests alone.

## CI and release checks

Configure `TEST_SPELEODB_INSTANCE` and `TEST_SPELEODB_OAUTH` under the
repository's **Settings → Secrets and variables → Actions → New repository
secret**. Use a running test instance and a valid token; no placeholder token
substitutes for real integration coverage. The release workflow remains manually
runnable from the Actions tab with **Run workflow**.

CI runs Rust/frontend lint on Ubuntu and native tests, frontend unit/browser
tests and native builds on Windows and macOS. Bun's version and lockfile are
pinned; dependencies are installed with `--frozen-lockfile`. Trusted pushes
require real API credentials so release gates cannot succeed with placeholder
credentials. Fork PRs may lack secrets and therefore skip real-network tests;
this limitation does not imply API coverage passed.

A successful tagged push dispatches the release workflow. Releases preserve
macOS/Windows packaging, draft status and updater artifacts. Run
`cargo test -p xtask --test release_workflow` after workflow edits; see
[Release workflow](docs/release-workflow.md). Existing Windows application
manifest tests remain necessary for native test executable startup.
