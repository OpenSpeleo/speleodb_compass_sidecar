# Tauri development server

## Feature intent

`cargo tauri dev` runs the desktop shell and a hot-reloading Yew frontend. Tauri
loads the frontend from `http://localhost:1420`, so Trunk and Tauri must agree
on a fixed local port.

## Design

The Tauri config uses `cargo run -p xtask -- trunk-serve-dev` as its
`beforeDevCommand`. That command checks port `1420` before starting
`trunk serve`.

If the port is already held by a stale `trunk` process, the preflight stops that
process and waits for the port to become free before launching a new Trunk
server. If another process owns the port, the command exits with an actionable
error instead of killing an unrelated process.

This keeps the normal development command simple while avoiding the common
failure mode where a previous dev run leaves Trunk listening on the fixed port.

## Verification

The process-detection and parser behavior is covered by `cargo test -p xtask`.
Full development verification is still `cargo tauri dev` from `app/` or
`make dev` from the repository root.

## Performance

The preflight performs one local port inspection during startup and then hands
off to Trunk. It does not run during application runtime.
