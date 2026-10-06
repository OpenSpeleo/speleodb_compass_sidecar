# React frontend architecture

## Intent and boundaries

React and TypeScript render the application UI. Bun manages packages and Vite
builds the frontend. The Rust backend remains authoritative for authentication,
polling, project state, permissions, file operations, import validation,
mutexes, menus and updates. The browser never calls SpeleoDB directly or gains
filesystem access.

The frontend has one typed controller for existing Tauri commands and one
subscription to `ui-state-update`. Wire names, argument keys, enum shapes and
error text remain compatible with Rust serialization. Register the subscription
before initialization and clean it up when the WebView closes or the store
module is replaced during development. Component remounting must not duplicate
the subscription or initialization side effects.

React owns transient form input, sorting, modals, import selection and focus.
Backend refreshes must not reset those interactions. Import selection computes
dependency closure from the preview graph without reading source files. The
backend still validates selected IDs and recomputes the authoritative closure
before writing; cross-language contract tests prevent frontend drift.

## Presentation and build

Retain the existing CSS, inline layout values, logo assets and SVG geometry. Do
not introduce a CSS reset, component design system or different default fonts.
The static `about.html` remains its own output and uses the global Tauri API, so
`withGlobalTauri` remains enabled. Its dependency credits describe the current
React frontend and native Rust libraries.

The native window remains 800×900. Vite outputs to `app/dist`; development uses
fixed port 1420. The explicit UI build profile preserves staging defaults for
debug and production defaults for release, independently of Vite mode. Native
build hooks infer that profile from `TAURI_ENV_PLATFORM` and `TAURI_ENV_DEBUG`;
standalone builds default to debug unless explicitly configured.
`app/package.json` is private and unversioned; application versions remain in
Cargo and Tauri configuration.

## Verification and performance

Persistent unit and component tests cover validation, ordering, error
presentation, updater behavior and import selection. Ten browser interaction
cases run in Chromium and WebKit, including native-dialog focus, scrolling and
large dependency lists. Rust/TypeScript contract fixtures protect serialization
and selection semantics.

Native macOS and Windows smoke testing completes coverage of operating-system
integration. See [Testing](../TESTING.md).

Frontend formatting covers every supported file under `app/`, including HTML,
CSS, JavaScript, TypeScript, and native configuration JSON. ESLint checks
JavaScript and TypeScript. Only generated build output, Tauri schemas, and test
reports are explicitly excluded; dependencies are ignored by the tools by
default. No source files are exempted to preserve a migration snapshot.

Removing Rust WASM compilation reduces frontend build dependencies. Keep
selectors pure and avoid graph/file rescans on each keystroke or background
update. Optimize repeated rendering only when measurements justify it and
interaction checks still pass. Native polling intervals and backend algorithms
stay intact.
