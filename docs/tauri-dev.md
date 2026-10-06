# Tauri development server

`make dev` runs `bun run tauri dev` from `app/`. Tauri starts the React frontend
through its Bun/Vite development hook and loads `http://localhost:1420`. Vite
uses a fixed port with `strictPort` so the native shell cannot accidentally
connect to a different application when another process holds that port.

Stop the listener explicitly before retrying. The development server does not
kill `node` or `bun` processes: those executables can belong to unrelated apps.
The previous Trunk-specific process recovery and its Rust launcher have been
removed. The versioning xtask discovers the repository through `Cargo.toml` and
`app/src-tauri/tauri.conf.json`, independently of frontend tooling.

Native code remains in `app/src-tauri`; frontend watching excludes native build
artifacts. Hot reload updates React while Rust changes use Tauri's normal
rebuild. This development plumbing adds no packaged runtime work.

Verify `make dev`, frontend changes, native rebuilds and an occupied-port
failure locally. `cargo test -p xtask` checks versioning and repository
discovery; `make build-ui` verifies the standalone production frontend.
