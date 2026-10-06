# Release workflow

`.github/workflows/publish.yml` builds Apple Silicon macOS and Windows packages
and creates a draft GitHub release. It runs on manual dispatch or when the CI
tagged-push gate dispatches a `v*` tag after all required checks pass.

## Reproducible frontend build

Both CI and release jobs install Node 24 and the Bun version declared in
`app/.bun-version`. Run `bun install --frozen-lockfile` from `app/`; cache Bun's
downloads by OS, Bun version and `app/bun.lock`, then build assets freshly. Do
not restore generated frontend output as a replacement for a build.

The Tauri action uses `projectPath: app` and `tauriScript: bun run tauri`.
`SIDECAR_UI_PROFILE: release` selects the production instance; output remains
`app/dist`, including the static About page and public assets. Local Tauri
builds also infer release/debug from Tauri's hook environment, so an ordinary
`bun run tauri build` selects production without a workflow-only override. The
workflow retains its existing app identity, draft release behavior, macOS ad-hoc
signing, Sentry input and updater signing secrets. `uploadUpdaterJson: true`
uploads updater metadata alongside signed release artifacts.

Rust caches remain rooted at the Cargo workspace. The Windows application
manifest setup is unchanged. CI runs native tests and unbundled native builds on
both supported systems before tagged-push release dispatch.

## Regression checks

```sh
cargo test -p xtask --test release_workflow
```

These tests protect pinned/frozen frontend installation, current project paths,
release/update signing configuration, platform coverage and the native/frontend
release gate. Workflow parsing tests cover both LF and CRLF checkouts.

The former Trunk bootstrap needed authenticated, forced and locked
cargo-binstall fallbacks after failures in release v26.6.10. Trunk and WASM
build tooling are no longer release dependencies, so those bootstrap workarounds
and their cache entries have been removed together. Existing Rust dependencies
are still locked; a frontend migration is not a reason to upgrade the native
dependency graph.
