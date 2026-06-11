# Paste Shortcuts Across App

## Checklist

- [x] Confirm root cause for Cmd/Ctrl+V paste failure.
- [x] Add a shared native Edit menu to every app menu state.
- [x] Preserve existing Account and Help menu behavior.
- [x] Add unit tests for authenticated and unauthenticated menu contents.
- [x] Run targeted Tauri tests.
- [x] Run lint if targeted tests pass.

## Review

Root cause was the app replacing Tauri's native menu with Account/Help-only
menus, which dropped the standard Edit menu and therefore desktop clipboard
accelerators. The fix centralizes menu construction and includes Edit in every
auth state while preserving existing Account and Help behavior.

Verification:

- `cargo test -p speleodb-compass-sidecar --lib`
- `make lint`
