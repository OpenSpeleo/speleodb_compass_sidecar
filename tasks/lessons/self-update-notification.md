# Lessons: Self-Update Notification

These lessons came out of an adversarial review of the first pass at the
self-update notification feature. They are written as rules for future
implementations of (a) the Tauri updater, (b) any feature that overlays
async workflows on a shared `UiState`, and (c) any "fire-and-forget"
notification flow.

## Tauri updater quirks

1. **`Update::install` on Windows calls `std::process::exit(0)`.** Anything
   you write after `install(...)` on Windows is unreachable. If you need a
   "Relaunching..." message in the UI, publish it _before_ the install call
   on Windows. Do not assume `app_handle.restart()` will run.
2. **`Update::install` is synchronous and can block for seconds.** On macOS
   it can spawn an `osakit` AppleScript dialog requesting admin privileges.
   Always wrap it in `tauri::async_runtime::spawn_blocking` so the async
   runtime keeps draining other tasks (UI emits, progress callbacks).
3. **`app_handle.restart()` from a non-main thread requests an exit and
   then sleeps the calling thread forever.** That is intentional: the main
   thread fires `RunEvent::ExitRequested`, runs cleanup hooks, and exits.
   Make sure the cleanup path (e.g. releasing project mutexes) is wired
   into `ExitRequested`, not into a flow that runs on the worker thread
   that is now blocked in `restart()`.
4. **`updater.check()` returning `Ok(None)` is the no-update case.** Don't
   conflate it with an error; only the `Err` path should publish a `Failed`
   notification.

## Workflow lifecycle (single-flight + signaling)

5. **A workflow lock that you set and unset by hand will eventually leak.**
   If anything between `swap(true)` and `store(false)` panics or returns
   early, the flag stays stuck and every subsequent check is silently
   dropped. Use an RAII guard (`Drop`-based) so the lock resets on _every_
   exit path — happy path, `?`-style early return, and panic.
6. **Single-flighting a workflow is fine, dropping user clicks on the floor
   is not.** When a manual user-facing action (e.g. a menu click) hits a
   busy lock, do not silently return. Either upgrade the running workflow
   to honor the user's intent (the pattern used here, where a
   `pending_manual_update_check` flag lets the no-update branch apply
   manual semantics) or queue a follow-up to run after the lock releases.
7. **Each workflow attempt deserves a fresh identity.** If dismissals are
   keyed by `id + phase`, a retry must allocate a new `id`. Otherwise an
   earlier `Failed` dismissal will silently suppress the retry's UI.

## Progress callbacks and out-of-order emits

8. **Spawning a task per progress callback creates an ordering hazard.**
   The chunk callback fires synchronously on the download thread, but each
   spawned publish lands on the async runtime in unspecified order. Without
   a monotonicity guard, a late 30% emit can overwrite a freshly published
   50% emit and the UI flickers backwards.
9. **Test the guard, not just the data structure.** Extract the
   "should-publish?" decision into a pure helper and unit-test it for
   `None`, equal, and regressing inputs. Asserting `record_chunk` returns
   the right percent is necessary but not sufficient.

## Loading state vs. ambient state

10. **`LoadingState` is for app initialization only.** Update activity is
    ambient and optional; it must not block auth or project loading.
    Keeping orthogonal concerns in separate state machines keeps each one
    debuggable. When a redesign moves a concern out of `LoadingState`,
    delete the dead variants — leaving them around invites future
    contributors to re-emit them and silently hang the app.

## Cosmetic / accessibility

11. **A red filled button reads as destructive.** "Retry" is the safe
    suggestion; reserve red fills for confirmable destructive actions and
    use red as an _accent_ (border, status dot) for the surrounding error
    state. Style the primary recovery action as a blue CTA.
12. **`cmd /C start "" URL` flashes a console window on Windows.** Always
    pass `CREATE_NO_WINDOW` (`0x0800_0000`) via `CommandExt::creation_flags`
    and silence stdio when shelling out from a GUI app.
13. **Use `×` (`\u{00D7}`) for dismiss glyphs**, not the ASCII letter `x`.
14. **Detach stdio on every `Command::spawn` from a GUI app**, not just
    on Windows. macOS `open` and Linux `xdg-open` work fine with inherited
    pipes today, but a future GUI subprocess that holds stdin open will
    keep the parent in a half-broken state at exit.
15. **`role="status"` implies `aria-atomic="true"`.** Drop a live region
    onto a label-plus-progress widget without overriding `aria-atomic` and
    every percent change re-announces the label. Either split the label
    out of the live region, or set `aria-atomic="false"` on the live
    region so screen readers announce only the changed text.

## Plugin-induced lifecycle leaks

16. **`process::exit` from a third-party plugin bypasses your
    `RunEvent::ExitRequested` handler.** Anything you wired into
    `ExitRequested` to release server-side state (mutexes, leases,
    websockets) is silently skipped. Audit every plugin that can exit the
    process — for `tauri-plugin-updater` that means `Update::install` on
    Windows — and register an `on_before_exit` (or equivalent) hook that
    runs your cleanup synchronously. Preserve the plugin's default hook
    by calling whatever it would have called (`cleanup_before_exit`)
    after your own cleanup so you do not regress its behaviour.
17. **`tauri::async_runtime::block_on` is safe from `spawn_blocking`
    threads but not from runtime worker threads.** `spawn_blocking`
    threads are part of Tokio's blocking pool and are *not* in async
    context, so they can drive a future to completion exactly like the
    main thread does inside a `RunEvent` handler. Keep cleanup logic in
    one of those two thread classes (or send to one via a channel).

## Test harness

18. **`Renderer::render` in CSR is asynchronous in practice; SSR is the
    cheaper test target.** When you need to verify rendered DOM in a Yew
    crate that ships with `csr` only, add `ssr` to the *dev-dependency*
    feature set so the production WASM bundle is unaffected.
    `LocalServerRenderer::render().await` returns a `String`, which is
    enough for substring/structure assertions and works under
    `wasm_bindgen_test` without any timer hacks.
