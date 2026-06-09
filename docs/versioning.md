# Versioning

## Intent

The app has two different version concepts:

- The user-facing application version, shown in the About window and used by
  release/update packaging.
- The local Compass metadata version, written to `compass.toml` inside each
  project working copy.

These should not share one constant. The app release can change frequently,
while the local metadata version should change only when the `compass.toml`
schema changes.

## Application version source

The user-facing version is the Tauri package version from
`app/src-tauri/tauri.conf.json`. Tauri codegen places that value into
`AppHandle::package_info().version`, and the `about_info` command returns that
package metadata to `app/about.html`.

Cargo package versions use the root `Cargo.toml` `[workspace.package].version`
as their single Cargo source of truth. Workspace member manifests inherit it
with `version.workspace = true`, so other package metadata consumers, including
`env!("CARGO_PKG_VERSION")`, do not report a different app version.

`tauri.conf.json.version` remains explicit for Tauri packaging. A backend unit
test compares it to `env!("CARGO_PKG_VERSION")` so JSON/TOML drift fails in the
normal Rust test suite.

Use the Cargo alias to bump both version files together:

```bash
cargo bump-version 27.6.9
```

The command updates root `Cargo.toml` and
`app/src-tauri/tauri.conf.json`. Date-like input with leading zeroes is accepted
but normalized to SemVer, so `cargo bump-version 27.06.09` writes `27.6.9`.

The startup logger prints `env!("CARGO_PKG_VERSION")` after file logging is
initialized, so `~/.compass/speleodb_compass*.log` records the running software
version.

## Local metadata version

`SPELEODB_COMPASS_TOML_VERSION` is intentionally fixed at `1.0.0` and scoped to
`app/src-tauri/src/project_management/local_project.rs`. It is serialized under
`[speleodb].version` in local `compass.toml` files created during Compass
project import.

That value is a metadata/schema marker, not the app release version. Only
change it when the local `compass.toml` format changes in a way that requires
version-aware handling.

## Verification

Backend coverage checks that:

- `about_info` uses Tauri package metadata from the generated app context.
- `app/src-tauri/tauri.conf.json.version` matches Cargo's package version.
- Imported Compass project metadata keeps `[speleodb].version` on
  `SPELEODB_COMPASS_TOML_VERSION`.

Run from repo root:

```bash
cargo test -p speleodb-compass-sidecar --lib about_info
cargo test -p speleodb-compass-sidecar --lib local_project
cargo test -p xtask
cargo fmt --all -- --check
make lint
```
