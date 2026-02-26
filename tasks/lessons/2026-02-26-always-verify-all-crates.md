# Lesson: Always verify ALL workspace crates before declaring done

**Date**: 2026-02-26
**Triggered by**: CI failure in `app/src/error.rs` -- a test referenced a deleted function

## What happened

1. We refactored `Error::ProjectImport` and removed the `is_permission_denied()` function from `app/src/error.rs`.
2. A cherry-pick auto-merged new tests calling that deleted function into `error.rs`.
3. We only ran the backend tests (`cargo test -p speleodb-compass-sidecar --lib`). Those passed.
4. We did not run the WASM UI check. CI failed.

## Root cause

Verifying only one crate and assuming the rest are fine. It doesn't matter why -- merge, cherry-pick, manual edit, refactor -- the rule is the same.

## Rule

**Before declaring any change done, always run the full verification for every workspace crate.** No shortcuts, no assumptions, no "I only touched one file". The minimal verification for this project is:

```bash
# Backend (native)
cargo test -p speleodb-compass-sidecar --lib

# Frontend (WASM)
cargo check --target wasm32-unknown-unknown --tests -p speleodb-compass-sidecar-ui
```

Both. Every time. No exceptions.
