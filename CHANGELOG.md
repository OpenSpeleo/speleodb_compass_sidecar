## [Unreleased]

## v26.7.26

### User-facing fixes

- Fixed the About window showing the hard-coded version v1.0.0.
- Consolidated Account, Help, editing, and application actions into one
  `SpeleoDB Compass Sidecar` menu on macOS and Windows.
- Preserved native Cut, Copy, Paste, and Select All actions and keyboard
  shortcuts without a separate Edit menu.
  - Removed macOS-injected Writing Tools, AutoFill, Dictation, and Emoji &
    Symbols menu entries, and normalized the menu name to space-separated title
    case.
- Restored the application Quit menu and standard Cmd+Q behavior on macOS.
- Fixed project loading when SpeleoDB returns unsupported or non-Compass project
  types.
- Non-Compass projects are now safely ignored instead of preventing the entire
  project list from loading.

### Versioning and development

- Centralized the application version across all Cargo workspace packages and
  Tauri packaging.
- Added cargo bump-version to update Cargo and Tauri versions together.
- Separated the application version from the fixed 1.0.0 Compass metadata-schema
  version.
- Added a development-server preflight that detects stale Trunk processes on
  port 1420.
- Removed a Windows `xtask` dead-code warning by compiling the Unix listener
  parser only on Unix.
- Improved integration-test diagnostics for unreachable servers and invalid
  OAuth credentials.
- Isolated backend test data from the user’s real ~/.compass directory.

### Dependencies

- Updated reqwest to 0.13.4.
- Updated sysinfo to 0.39.
- Relaxed the direct wasm-bindgen constraint while retaining lockfile
  compatibility.
- Updated the ESLint pre-commit mirror to 10.4.1.
- Refreshed Cargo dependencies and lockfile entries.

### CI and release tooling

- Added reliable cross-platform WASM UI testing on Windows and macOS.
- Ensured wasm-bindgen-cli matches the project’s locked wasm-bindgen version.
- Fixed geckodriver discovery on Windows and authenticated its macOS download.
- Authenticated cargo-binstall requests to prevent GitHub API rate-limit
  failures.
- Added caching for Cargo tools, Trunk, wasm-pack, wasm-bindgen, and prek.
- Corrected Trunk cache paths and cache invalidation inputs.
- Avoided reinstalling cached tools when compatible binaries already exist.
- Made workflow regression tests reliable on Windows CRLF checkouts.
- Pinned cargo-binstall Actions usage instead of tracking its moving master
  branch.
- Expanded Dependabot coverage to every Cargo package in the workspace.
- Streamlined pre-commit checks to remove repeated compile, documentation, and
  test work.

### Documentation and maintenance

- Added documentation for application versioning and version bumps.
- Added documentation for native desktop menu requirements.
- Added documentation for Tauri development-server port handling.
- Updated API v2, testing, update-notification, Windows manifest, logging, and
  contributor documentation.
- Normalized documentation formatting and Compass test fixtures.

## v26.6.10
