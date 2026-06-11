# Tauri dev port collision

## Plan

- [x] Add a durable development preflight for the fixed Trunk/Tauri port.
- [x] Wire `cargo tauri dev` through the preflight without changing the dev URL.
- [x] Document why the port is fixed and how stale dev servers are handled.
- [x] Verify with focused tests and lint/build checks where feasible.

## Review

- Added `cargo run -p xtask -- trunk-serve-dev` as the Tauri
  `beforeDevCommand`.
- The preflight frees port `1420` only when the listener is clearly Trunk; other
  processes produce an actionable error.
- Documented the fixed-port development workflow in `docs/tauri-dev.md`.
- Verified with `cargo test -p xtask` and `make lint`.
- Cleared the stale local `trunk` process that was listening on port `1420`.
