# WASM tool cache repair

## Diagnosis

- [x] Identify the failing command and distinguish it from the passing Rust
      tests.
- [x] Confirm that the Cargo tool cache restored installation metadata without
      the `wasm-pack` binary.
- [x] Confirm that plain `cargo binstall` trusted the stale metadata and exited
      successfully without restoring the missing binary.

## Implementation

- [x] Add a regression test for the CI WASM tool bootstrap.
- [x] Force-install `wasm-pack` only when its executable is missing.
- [x] Document the CI tooling fix in the changelog.

## Verification

- [x] Run the focused workflow regression test.
- [x] Validate the CI workflow YAML.
- [x] Run Rust formatting and lint checks.
- [x] Run whitespace checks and inspect final staged/unstaged state.

## Review

The macOS job restored a Cargo tool cache containing `.crates2.json` but no
`wasm-pack` executable. The bootstrap correctly noticed that the executable was
missing, but plain `cargo binstall` trusted the stale metadata, reported that
`wasm-pack` was already installed, and exited without repairing the binary.
`make test-ui-ci` then failed with exit 127.

The guarded bootstrap now uses `--force` when `command -v wasm-pack` fails. This
retains the cache fast path when the binary exists while repairing
metadata-vs-binary skew. A workflow regression test preserves the forced, locked
installation command.

Final verification:

- Focused regression test: passed after reproducing the failure before the fix
- `cargo test -p xtask --test release_workflow`: 3 passed
- `cargo test -p xtask`: 10 passed
- `prek run check-yaml --files .github/workflows/ci.yml`: passed
- `cargo fmt --all -- --check`: passed
- `make lint`: passed
- `git diff --check`: passed

The workspace began clean with no staged files. All final changes are
task-related and remain unstaged.
