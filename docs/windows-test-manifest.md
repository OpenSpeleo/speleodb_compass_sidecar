# Windows Application Manifest for Test Binaries

## Intent

Every linked artifact produced by the `speleodb-compass-sidecar` crate
(`[lib]`, `[[bin]]`, examples, benches, **and unit/integration test
exes**) must ship the same Windows application manifest declaring a
dependency on Microsoft Common Controls v6. Without it, the Windows PE
loader serves the legacy ComCtl5 stub, which deliberately omits v6
entry points and aborts the binary at startup with
`STATUS_ENTRYPOINT_NOT_FOUND` (`0xc0000139`) the moment any Tauri,
`tauri-plugin-dialog`, or `tauri-plugin-updater` symbol appears in its
import table.

## Why this workaround exists

Stock `tauri-build` v2.5.6 attaches its default manifest only to
artifacts declared as `[[bin]]`. It does so by emitting
`cargo:rustc-link-arg-bins=...` from inside `embed-resource`, and Cargo
applies that link arg only to bins, never to test binaries. The gap
exists upstream:

- [tauri#13419 — `STATUS_ENTRYPOINT_NOT_FOUND` when running `cargo test` on Windows](https://github.com/tauri-apps/tauri/issues/13419)
- [tauri#13948 — `STATUS_ENTRYPOINT_NOT_FOUND` on windows when running app imported into a crate as a library](https://github.com/tauri-apps/tauri/issues/13948)
- [tauri#14580 — `STATUS_ENTRYPOINT_NOT_FOUND` when using `tauri::Window` in library tests on Windows](https://github.com/tauri-apps/tauri/issues/14580)
- [embed-resource#69 — `compile()` only links the manifest to bins](https://github.com/nabijaczleweli/rust-embed-resource/issues/69)

All three Tauri issues remain open as of the time of writing. The fix
endorsed by the Tauri lead (`lucasfernog`) and confirmed working by
multiple reporters is to take ownership of the manifest pipeline in our
own `build.rs` and emit `cargo:rustc-link-arg=...` (without the `-bins`
suffix) so the manifest reaches every linked artifact.

## How it is implemented

[`app/src-tauri/build.rs`](../app/src-tauri/build.rs):

1. Calls `tauri_build::WindowsAttributes::new_without_app_manifest()` to
   suppress `tauri-build`'s bin-only manifest. This guarantees a single
   canonical manifest in the linked image — no `LNK4078`-style
   duplicate-section warnings.
2. Calls `embed_resource::compile_for_everything(...)` on
   [`windows-app-manifest.rc`](../app/src-tauri/windows-app-manifest.rc),
   which references
   [`windows-app-manifest.xml`](../app/src-tauri/windows-app-manifest.xml).
   `compile_for_everything` (vs. plain `compile`) is the upstream API
   that emits `cargo:rustc-link-arg=...` (no `-bins` suffix), reaching
   bins, examples, benches, and tests in one pass.
3. Uses `.manifest_required().unwrap()` rather than `.manifest_optional()`
   so a missing `RC.EXE` / `windres` toolchain becomes a hard build
   failure rather than a silent skip — the manifest is loader-critical,
   not cosmetic.

`embed-resource` is scoped via
`[target.'cfg(windows)'.build-dependencies]` so non-Windows hosts
(macOS, Linux dev) do not pay the compile cost for an unused crate. It
is already a transitive dependency through `tauri-winres`, so promoting
it to a direct build-dep does not change `Cargo.lock`.

## What is in the manifest

[`windows-app-manifest.xml`](../app/src-tauri/windows-app-manifest.xml)
is byte-for-byte equivalent to `tauri-build` v2.5.6's default — only the
ComCtl v6 dependency declaration, nothing else. Behavior of release
builds is unchanged. If the manifest is ever extended (DPI awareness,
long-path support, requested execution level, etc.) update both the XML
file and this document.

## Toolchain coverage

`embed-resource` handles the toolchain split internally:

- MSVC (`x86_64-pc-windows-msvc`, used by GitHub Actions
  `windows-latest`): invokes `RC.EXE` from the Windows SDK.
- mingw GNU (`x86_64-pc-windows-gnu`, documented as the local Windows
  dev toolchain in [`AGENTS.md`](../AGENTS.md)): invokes `windres`.
- Cross-compilation from Linux/macOS: invokes `llvm-rc`, configurable
  via the `RC` / `RC_<TARGET>` environment variables.

No additional CI plumbing is required.

## When this can be reverted

Once any one of the following lands and is in the version we depend on:

- [tauri-winres switches to `compile_for_everything`](https://github.com/tauri-apps/tauri/issues/13419#issuecomment-2107228066)
- A new `WindowsAttributes` knob is added to `tauri-build` that links
  the manifest to test binaries.

Verification on revert: delete `windows-app-manifest.{xml,rc}`, the
`embed-resource` build-dep, and the `#[cfg(windows)]` branch in
`build.rs`, then confirm that
`cargo test --workspace --exclude speleodb-compass-sidecar-ui` passes
on the `windows-latest` GitHub runner. If it fails with `0xc0000139`
again, the upstream fix has not actually shipped — restore this
workaround.

## Verification checklist (must hold after any change here)

1. `cargo check --workspace --all-features` passes on macOS and Linux.
2. `make lint-clippy` passes on macOS (`-D warnings`).
3. `cargo test --workspace --exclude speleodb-compass-sidecar-ui`
   completes the `speleodb_compass_sidecar_lib` test binary without
   exit code `0xc0000139` on `windows-latest`.
4. `cargo tauri build` on Windows produces a release `.exe` with
   exactly one embedded manifest. Quick check from a Developer Command
   Prompt:
   `mt.exe -inputresource:target\release\<exe>;#1 -out:NUL` should
   succeed and report a single manifest. Visual sanity: dialogs and the
   file picker render with v6 styling, identical to today's release
   builds.
