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

The root `Cargo.toml` `[workspace.package].version` is the single application
version source. Workspace member manifests inherit it with
`version.workspace = true`.

`app/src-tauri/tauri.conf.json` deliberately omits `version`. Tauri then uses
the resolved Cargo package version for packaging and code generation, including
`AppHandle::package_info().version`. The `about_info` command returns that
package metadata to `app/about.html`, and the updater uses the same app version.
There is no JSON version to synchronize or override Cargo accidentally.

Release preparation resolves the Sidecar member through `cargo metadata`.
Resolution happens at build time and adds no runtime work.

Use the Cargo alias to bump the application version:

```bash
cargo bump-version 27.6.9
```

The command updates only root `Cargo.toml`; the Tauri configuration stays
versionless. Date-like input with leading zeroes is accepted but normalized to
SemVer, so `cargo bump-version 27.06.09` writes `27.6.9`.

For the complete agent release workflow, use the repository's
[`bump-version` skill](../.agents/skills/bump-version/SKILL.md), for example:
"Bump the version to 27.6.9 using $bump-version." It directly edits the root
Cargo version, runs `cargo update --workspace --offline` to refresh inherited
workspace versions without upgrading unrelated dependencies, and moves
Unreleased notes under `## v27.6.9` with a fresh empty `## [Unreleased]` above
it. The three metadata files (`Cargo.toml`, `Cargo.lock` and `CHANGELOG.md`)
belong in one `[compass_speleodb] Release - v27.6.9` commit when the user
explicitly requests staging and committing them.

This metadata-only workflow uses file and diff inspection instead of builds,
tests, or commit hooks, so it avoids compiling `xtask` or the application. It
has no runtime performance impact and does not tag, push, or publish a release.
The verification commands below remain relevant when changing the versioning
implementation itself.

The startup logger prints `env!("CARGO_PKG_VERSION")` after file logging is
initialized, so `~/.compass/speleodb_compass*.log` records the running software
version.

The frontend `app/package.json` is private and deliberately has no version
field. Bun dependency changes do not create another application version source.

## Local metadata version

`SPELEODB_COMPASS_TOML_VERSION` is intentionally fixed at `1.0.0` and scoped to
`app/src-tauri/src/project_management/local_project.rs`. It is serialized under
`[speleodb].version` in local `compass.toml` files created during Compass
project import.

That value is a metadata/schema marker, not the app release version. Only change
it when the local `compass.toml` format changes in a way that requires
version-aware handling.

## Verification

Backend coverage checks that:

- `about_info` uses Tauri package metadata.
- Imported Compass project metadata keeps `[speleodb].version` on
  `SPELEODB_COMPASS_TOML_VERSION`.

Run from repo root:

```bash
cargo test -p speleodb-compass-sidecar --lib commands::tests
cargo test -p speleodb-compass-sidecar --lib local_project
cargo test -p xtask
python -B -m unittest discover -s scripts/release -p test_project_version.py
cargo fmt --all -- --check
prek run --all-files
```
