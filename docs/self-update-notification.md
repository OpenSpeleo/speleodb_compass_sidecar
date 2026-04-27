# Self-Update Notification

## Intent

The app checks for `tauri-plugin-updater` releases automatically once per app
session. Update activity must never block authentication or project loading,
and the user should only see updater UI while work is happening, when a newer
release is being applied, or when recovery is needed.

## Why this design exists

Bundling updates into [`LoadingState`](../common/src/ui_state.rs) coupled
the auth/project-loading state machine to network I/O against the update
server. A failing endpoint, a slow signature check, or an unexpected installer
error blocked the user from ever reaching the auth screen. The new design
separates the two concerns:

- `LoadingState` describes app initialization only (`NotStarted`,
  `LoadingPrefs`, `Authenticating`, `LoadingProjects`, `Unauthenticated`,
  `Ready`, `Failed`).
- An optional `UpdateNotification` rides alongside on the same `UiState` and
  drives a small bottom-right toast that is independent of every other
  screen.

## Notification model

`UpdateNotification` carries a numeric `id` (per workflow) and an
`UpdateNotificationPhase` (`Checking`, `Downloading`, `Installing`,
`Relaunching`, `UpToDate`, `Failed`). The `dismissal_key()` is
`{id}:{phase_kind}` — _not_ keyed by progress percent — so dismissing the
`Downloading` phase silences every percent update for that workflow but a
later `Installing` or `Failed` phase can still reappear.

A new manual `Check for Updates Now` always allocates a fresh `id`, which
guarantees dismissal of an older workflow's `Failed` notification cannot
suppress the retry's UI.

## Workflow lifecycle

`AppState` owns three coordination primitives:

| flag                                | role                                                                  |
| ----------------------------------- | --------------------------------------------------------------------- |
| `startup_update_check_started`      | one-shot guard so the startup check fires at most once per session    |
| `update_workflow_running`           | mutual-exclusion lock between concurrent check workflows              |
| `pending_manual_update_check`       | "a manual click landed during a running workflow" signal              |

A workflow is spawned only by the function that **claims**
`update_workflow_running` via `swap(true)`; the spawned task wraps the
running flag in a `WorkflowGuard` (`Drop`-based RAII) so the flag is reset on
the happy path, on `?`-style early returns, and on panic alike. After the
guard drops, the spawn checks `pending_manual_update_check`; if it was set,
it kicks off a fresh manual check so the user always sees feedback.

### Origin handling

Update checks have one of two origins:

- `Startup` — automatic; a no-update outcome silently clears the toast.
- `Manual` — invoked from `Help → Check for Updates Now` (or the failure
  toast's `Retry` button); a no-update outcome shows
  `<AppName> is up to date.` for ~4s before auto-clearing.

If a manual click arrives while a `Startup` workflow is in flight,
`pending_manual_update_check` is set. The running workflow consumes the flag
in its no-update branch and upgrades the effective origin to `Manual` so the
manual semantics are honored. Any leftover signal (e.g. the manual click
arrived after the no-update branch already executed) triggers a follow-up
manual check once the running workflow releases the lock.

### Frontend dismissal

Each notification carries its `dismissal_key`. The frontend sends the key
back via the `dismiss_update_notification` command, which adds it to a
backend set. While the key is in the set, no future notification with the
same key will be republished. Phase changes use a new key, so an
`Installing` or `Failed` notification still surfaces after a `Downloading`
dismissal.

## Download progress and out-of-order callbacks

The Tauri updater download stream invokes our chunk callback synchronously,
but each progress publish is dispatched via `tauri::async_runtime::spawn`,
which means the publishes can land out of order on a multi-threaded runtime.
`publish_download_progress_notification` therefore consults
`should_publish_progress(current, new)` and only commits the new value when
it strictly advances the displayed percent. A late 30% emit cannot regress
the UI from a freshly published 50%.

## Install and restart, by platform

`tauri_plugin_updater::Update::install` is **synchronous** and platform
behavior diverges sharply at this step:

- **Windows** — `install_inner` extracts the archive, launches the bundled
  NSIS or MSI installer via `ShellExecuteW`, then calls
  `std::process::exit(0)`. The current process disappears; the installer is
  responsible for relaunching the app afterwards. Any post-install code
  (including `app_handle.restart()`) is unreachable. To preserve the spec's
  `Installing → Relaunching` sequence, we publish `Relaunching` _before_
  invoking `install` on Windows.

  Because `process::exit(0)` skips Tauri's event loop entirely, the
  `RunEvent::ExitRequested` handler in `lib.rs` that releases the active
  project mutex never fires. To plug that leak we register a custom
  `on_before_exit` hook on the per-call `UpdaterBuilder` that runs the
  project-mutex release synchronously (via
  `tauri::async_runtime::block_on`) and then calls
  `app_handle.cleanup_before_exit()` to preserve the plugin's default
  cleanup behaviour. The plugin invokes the hook just before `process::exit`,
  so the mutex is gone before the installer takes over.

- **macOS** — `install_inner` extracts the new `.app` to a temp dir, swaps
  it into place (sometimes via an `osakit` AppleScript with admin
  privileges), and returns. We then publish `Relaunching`, sleep ~150ms so
  the WebView has time to render the final phase, and call
  `app_handle.restart()`. From a non-main thread `restart()` requests an
  exit with `RESTART_EXIT_CODE`; the main thread fires
  `RunEvent::ExitRequested` (whose `prevent_exit` is ignored for restarts)
  and our handler in `lib.rs` releases any held project mutex before the
  runtime relaunches the binary.

- **Linux** — `install_inner` rewrites the AppImage in place (or installs a
  `.deb` via `pkexec`/`zenity`/`sudo`) and returns. We publish `Relaunching`
  with the same short sleep before `app_handle.restart()`, which behaves as
  on macOS.

Because install does real file I/O (and on macOS prompts the user via
AppleScript), it is dispatched through `tauri::async_runtime::spawn_blocking`
so the async runtime keeps draining other tasks (UI emits, progress
publishes) while it runs.

## Failure recovery

Any failure between `check`, `download`, and `install` (network error,
bad signature, permission denied, malformed archive) is mapped to
`UpdateNotificationPhase::Failed { message }` and surfaced in the toast.
The toast renders red accent + status dot, the human-readable error,
and two recovery actions:

- `Retry` calls `start_manual_update_check`, which allocates a new workflow
  `id` (so previous `Failed` dismissals do not suppress the retry).
- `Download Latest` opens the public GitHub releases page via the OS
  default browser. The Windows path uses `CREATE_NO_WINDOW` to suppress a
  flashing `cmd` console; every platform also detaches stdio so the
  helper process does not inherit the parent's pipes.

## Accessibility

The toast is a discoverable but non-intrusive ambient region. The
outer `<aside>` carries an `aria-label="Update status"` so it shows up
in screen-reader landmark navigation, but it is **not** itself a live
region. Only the label-plus-message wrapper carries
`role="status" aria-live="polite" aria-atomic="false"`:

- `polite` so phase transitions and the appearance of the toast are
  announced without preempting other speech.
- `aria-atomic="false"` so download-progress updates announce only the
  changed message text — without this override the implicit
  `aria-atomic="true"` of `role="status"` would re-read the static
  "Updates" label every percent.

The dismiss button and recovery actions sit outside the live region, so
their appearance is not announced as text; users navigate to them via
Tab as with any other button.

## Verification

Backend helpers covered:

- progress accumulation, content-length-zero, missing content-length, and
  100% clamping (`DownloadProgress::record_chunk`)
- the `should_publish_progress` monotonicity guard (rejecting
  equal/regressing percents)
- origin → no-update notification mapping
- error message humanization (empty, padded, actionable)

Shared `UiState` covered:

- dismissal key stability across `Downloading` percent updates
- dismissal key change across phase transitions
- workflow `id` increments per check (so retries are not suppressed)

Frontend helpers covered:

- the message string for every phase
- which phases render as "working" (spinner) versus "error" (dot + actions)

Run from repo root:

```bash
cargo fmt --all -- --check
cargo test -p common
cargo test -p speleodb-compass-sidecar --lib
make test-ui
```
