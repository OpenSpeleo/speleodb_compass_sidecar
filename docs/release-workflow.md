# Release workflow

`.github/workflows/publish.yml` builds Apple Silicon macOS and Windows packages
and creates a draft GitHub release. It runs on manual dispatch or when the CI
tagged-push gate dispatches a `v*` tag after all required checks pass. The flow
adapts FrameForge's prepare, sign, verify and aggregate stages to Sidecar's
workspace and supported platforms. Signing setup belongs to the operator; the
release workflow consumes existing credentials and never provisions them.

## One draft and two platform builds

The preparation job resolves the `speleodb-compass-sidecar` package version
through Cargo metadata and checks it against the Tauri configuration. It
requires the dispatched tag to match that version, finds an existing draft
through the paginated release inventory, or creates one. A published release or
duplicate matching releases stops the job. Both build jobs receive the same
release ID, and the final aggregation job rechecks the draft's identity before
uploading.

This coordination prevents parallel platform builds from producing competing
drafts or publishing a partial updater manifest. Concurrency serializes release
runs, and every successful run leaves the release as a draft for review.

| Build          | GitHub environment | Release directory                     |
| -------------- | ------------------ | ------------------------------------- |
| macOS ARM64    | `macos-signing`    | `target/aarch64-apple-darwin/release` |
| Windows x86_64 | `windows-signing`  | `target/release`                      |

The macOS target remains explicit. Every artifact discovery, notarization,
verification and metadata step uses the corresponding release directory. This
repository does not include FrameForge's Linux or multimedia SDK build stages.

## Reproducible frontend build

Both CI and release jobs install the Bun version declared in `app/.bun-version`.
Run `bun install --frozen-lockfile` from `app/`; cache Bun's downloads by OS,
Bun version and `app/bun.lock`, then build assets freshly. Do not restore
generated frontend output as a replacement for a build.

The retained browser workflow template at `app/.github/workflows/playwright.yml`
follows the same Bun runtime contract; GitHub executes only the workflows in the
repository-root `.github/workflows`.

`app/bunfig.toml` forces scripts and executable children onto Bun, including
tools with Node shebangs. Browser installation explicitly uses
`bunx --bun --no-install`. Workflows do not install a separate Node runtime;
GitHub manages the internal runtime of its JavaScript actions independently.
Native Rust, platform SDKs and signing remain required. This removes duplicate
JavaScript runtime setup without changing the native dependency graph or the
packaged application's WebView runtime.

The Tauri action uses `projectPath: app` and `tauriScript: bun run tauri`.
`SIDECAR_UI_PROFILE: release` selects the production instance; output remains
`app/dist`, including the static About page and public assets. Local Tauri
builds also infer release/debug from Tauri's hook environment, so an ordinary
`bun run tauri build` selects production without a workflow-only override. The
workflow retains its existing app identity, Sentry input and updater signing
secrets. Operating-system signing and updater signing serve separate purposes:
the former establishes the publisher for the OS, while the latter verifies
downloads inside Sidecar using the existing updater public key.

Rust caches remain rooted at the Cargo workspace. The Windows application
manifest setup is unchanged. CI runs native tests and unbundled native builds on
both supported systems before tagged-push release dispatch.

## Code signing

### macOS

The macOS job imports the existing Developer ID Application certificate using
`apple-actions/import-codesign-certs`. It writes the App Store Connect API
private key to a restricted temporary file and exports `APPLE_API_KEY_PATH`, key
ID, issuer and signing identity for Tauri. The workflow always removes the
temporary key after the notarization steps. The import action owns its temporary
keychain.

Tauri signs, notarizes and staples the application before packaging its updater
archive. The workflow separately submits the final DMG to Apple's notary
service, waits with a bounded timeout, and staples the accepted ticket.
Verification mounts the actual DMG, checks the contained application's code
signature, Gatekeeper assessment and ticket, and checks the DMG's Gatekeeper
assessment and ticket. The mount is detached on success or failure.

The Tauri action initially uploads the DMG before its ticket is stapled. Only
after notarization and all local artifact checks succeed does the workflow
replace that draft asset with the stapled DMG. It does not modify the separately
signed `.app.tar.gz` updater payload. A failed notarization or verification
leaves the release unfinished as a draft; do not publish its partial assets.

The `macos-signing` environment must expose `APPLE_CERTIFICATE`,
`APPLE_CERTIFICATE_PASSWORD`, `APPLE_SIGNING_IDENTITY`, `APPLE_API_KEY`,
`APPLE_API_ISSUER` and `APPLE_API_PRIVATE_KEY`. The API key path is generated by
CI, not stored as another secret. See [macOS setup](macos-signing-setup.md) for
certificate export, credential rotation and delayed notarization handling.

### Windows

The Windows job grants `id-token: write` and uses `azure/login` with the
`windows-signing` environment. The existing Entra application must trust
`repo:OpenSpeleo/speleodb_compass_sidecar:environment:windows-signing` and have
the signing role at the certificate-profile scope. Secrets alone do not create
that federation or role assignment.

The job caches the pinned ArtifactSigning PowerShell module and dependencies,
then supplies a temporary Tauri `bundle.windows.signCommand` configuration.
`scripts/release/windows-sign.ps1` uses the Azure CLI session, checks for an
available signing token, and renews the GitHub OIDC assertion if a long build
outlived the earlier login. It signs with SHA256 and an RFC3161 timestamp and
requires a valid Authenticode signature with a timestamp certificate before
returning success.

Signing occurs during bundling, before Tauri generates updater signatures and
uploads artifacts. Local Windows builds keep their existing configuration. Never
re-sign a completed installer after its updater signature is generated.

The environment must expose `AZURE_CLIENT_ID`, `AZURE_TENANT_ID`,
`AZURE_SUBSCRIPTION_ID`, `AZURE_SIGNING_ENDPOINT`, `AZURE_SIGNING_ACCOUNT` and
`AZURE_SIGNING_PROFILE`. See [Windows setup](windows-signing-setup.md) for
shared publisher resources and repository-specific federation.

## Updater verification and metadata

Each platform checks its local payloads and signature files, then runs the
shipping executable with `--verify-updater-runtime`. This mode runs before
Sentry and desktop startup. A Python helper serves only the selected artifacts
and metadata on a loopback HTTP server; the native updater downloads each
target, verifies its signature using the application's public key, and compares
the downloaded bytes with the original file. It never installs the update.

The mode consumes `SIDECAR_UPDATER_SMOKE_ENDPOINT`,
`SIDECAR_UPDATER_SMOKE_TARGETS` and `SIDECAR_UPDATER_SMOKE_ASSETS`. HTTP is
enabled only for this headless context after restricting the endpoint to
loopback. Proxies and redirects are disabled. The HTTP client applies the same
120-second timeout to metadata requests and package downloads; setting only the
updater builder's timeout does not bound its separate download client. Artifact
names are encoded consistently in URLs and the asset map so Sidecar's filenames
containing spaces work correctly. Normal desktop startup and its updater
configuration remain unchanged.

Platform workers generate independent metadata fragments from the original local
signatures and the draft release's uploaded asset inventory. The final job
requires both fragments, validates their version, release notes, URLs, asset
names and signatures, and uploads a single `latest.json` containing these keys:

| Updater target                         | Payload                     |
| -------------------------------------- | --------------------------- |
| `darwin-aarch64`, `darwin-aarch64-app` | macOS `.app.tar.gz` archive |
| `windows-x86_64`, `windows-x86_64-msi` | Windows MSI installer       |
| `windows-x86_64-nsis`                  | Windows NSIS installer      |

The generic Windows entry preserves MSI compatibility, while NSIS has its own
signature and URL. The Tauri action does not upload `latest.json` or individual
signature files; signatures are embedded in the aggregated manifest. The
existing updater endpoint and public key stay authoritative.

Signing, notarization and the loopback downloads add release-time work. Signing
tools and build dependencies are cached, and the smoke server serves original
payloads without copying them. Byte comparison uses a fixed-size buffer in
addition to the updater plugin's download buffer. Normal application startup
does not execute these verification operations.

## Regression checks

```sh
cargo test -p xtask --test release_workflow
uv run --no-project python -B -m unittest discover -s scripts/release
cargo test -p speleodb-compass-sidecar --bin speleodb-compass-sidecar --test updater_smoke --locked
prek run --all-files
```

On Windows, also run the isolated signing callback tests:

```powershell
pwsh -NoProfile -NonInteractive -File scripts/release/test_windows_sign.ps1
```

Release helpers use `uv run --no-project` so invoking them from a monorepo
checkout cannot synchronize an ancestor's unrelated Python environment.

These tests protect pinned/frozen frontend installation, current project paths,
release/update signing configuration, platform coverage and the native/frontend
release gate. They also guard Bun script inheritance and the Bun formatter hook
against reintroducing Node setup. Frontend runtime tests verify the actual
Vitest worker and inherited executable runtime. Workflow parsing tests cover
both LF and CRLF checkouts.

Release helper tests cover artifact discovery, version selection, per-platform
fragments, complete aggregation, incorrect draft inventories, missing or
malformed signatures, and filenames containing spaces. Native tests exercise
signed downloads, tampered bytes, invalid metadata and loopback restrictions.
They also verify that a package response which stalls after successful metadata
retrieval times out without hanging the test fixture. Signing callback checks
protect authentication renewal and timestamp validation.

These deterministic checks do not prove Azure permissions, Apple credential
validity or successful hosted signing. Qualification requires the hosted Windows
job to produce valid timestamped signatures and the macOS job to pass Apple
notarization, Gatekeeper and ticket checks. Both jobs must pass updater smoke
verification before the complete metadata is uploaded. Failed jobs may leave
partial assets in the draft; rerun the same eligible tag after correcting the
failure and review the completed draft before publishing.

The former Trunk bootstrap needed authenticated, forced and locked
cargo-binstall fallbacks after historical release failures. Trunk and WASM build
tooling are no longer release dependencies, so those bootstrap workarounds and
their cache entries have been removed together. Existing Rust dependencies are
still locked; a frontend migration is not a reason to upgrade the native
dependency graph.
