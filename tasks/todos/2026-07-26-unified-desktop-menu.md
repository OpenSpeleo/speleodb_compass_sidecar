# Unified Desktop Menu

## Plan

- [x] Replace the separate Application, Account, Edit, and Help top-level menus
      with one flat `SpeleoDB Compass Sidecar` menu.
- [x] Preserve Sign Out as an authenticated-only action.
- [x] Preserve native Cut, Copy, Paste, Select All, and Quit roles.
- [x] Disable macOS-injected Writing Tools, AutoFill, Dictation, and Emoji &
      Symbols entries and lock the space-separated title-case menu name.
- [x] Add exact authenticated and unauthenticated menu-layout tests.
- [x] Update current desktop-menu, update-notification, agent, and lesson
      documentation.
- [x] Run focused tests, lint, the broader test suite, and Compass builds.
- [x] Manually verify the native menu behavior on macOS.
- [x] Manually verify the native menu behavior on Windows.

## Review

- Replaced four top-level menus with one flat application menu on macOS and
  Windows while retaining native clipboard and Quit roles.
- Kept Sign Out conditional on authentication and shared custom action IDs with
  the event dispatcher.
- Added macOS bundle settings and matching runtime defaults for Writing Tools,
  AutoFill, Dictation, and Emoji & Symbols, then corrected the implementation
  again to remove non-Sidecar selectors after every menu install because macOS
  still injected the entries.
- Compile-time isolated that cleanup from Windows: the module, call sites, and
  Objective-C/AppKit dependencies exist only for `target_os = "macos"`; there is
  no Windows no-op implementation to compile.
- Locked `SpeleoDB Compass Sidecar` as the space-separated menu and bundle name
  and title-cased `Check For Updates Now`.
- Updated desktop-menu architecture, update navigation, agent guidance, and the
  platform-role lesson.

Verification:

- `cargo test -p speleodb-compass-sidecar menu --lib` — 6 passed.
- `cargo tree -p speleodb-compass-sidecar --target x86_64-pc-windows-msvc` —
  contained no `objc2`, AppKit, or Foundation dependency.
- `make lint` — passed.
- Rust portions of `make test` — passed; 4 pre-existing preference tests
  remained ignored.
- `make test-ui` — 33 passed in headless Firefox.
- `NO_COLOR=true make build-ui` — passed.
- `NO_COLOR=true cargo tauri build --no-bundle` — passed.
- `NO_COLOR=true cargo tauri build --bundles app --no-sign` — passed.
- `make check-rust` from the monorepo root — passed with pre-existing
  `openspeleo_core` benchmark deprecation warnings.
- `plutil` validated the source plist and confirmed the built app has the
  correct bundle/display names plus all three suppression keys set to `true`.

Manual acceptance:

- Confirmed the exact rendered menu, authentication transitions, and clipboard
  shortcuts on macOS and Windows.
