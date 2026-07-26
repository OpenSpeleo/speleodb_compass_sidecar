# Release Trunk cache repair

## Diagnosis

- [x] Trace the release failure to Cargo install metadata being restored without
      the corresponding `trunk` executable.
- [x] Confirm that plain `cargo binstall` trusts the stale metadata, exits
      successfully, and leaves the following Tauri build unable to run Trunk.

## Implementation

- [x] Add regression coverage requiring the release bootstrap to repair a
      missing Trunk binary.
- [x] Force a locked Trunk install only when its executable is missing.
- [x] Document the cache failure mode and release safeguard.
- [x] Record the CI tooling fix in the changelog.

## Verification

- [x] Reproduce the missing safeguard with the focused workflow test.
- [x] Run the focused release-workflow regression test after the fix.
- [x] Run the complete `xtask` test suite.
- [x] Validate workflow YAML, Rust formatting, lint, and whitespace.
- [x] Inspect staged and unstaged state separately after the work.

## Review

The release job restored Cargo's install records and then `cargo-binstall`
reported Trunk 0.21.14 as already installed even though `command -v trunk` had
failed. The install step consequently exited zero without creating the
executable, and Tauri's `beforeBuildCommand` failed with
`trunk: command not found`.

The guarded recovery now uses `--force` to bypass stale install metadata while
retaining `--locked`, step-scoped GitHub authentication, and the explicit Bash
shell. The fast path remains unchanged when the Trunk executable is present.

Final verification:

- Focused regression reproduced the missing safeguard before the workflow fix.
- `cargo test -p xtask --test release_workflow`: 4 passed
- `cargo test -p xtask`: 11 passed
- `prek run check-yaml --files .github/workflows/publish.yml`: passed
- `cargo fmt --all -- --check`: passed
- `make lint`: passed
- `git diff --check` and `git diff --cached --check`: passed

The workspace began clean with no staged files. All final changes are
task-related, unstaged, and untracked only where expected for this task record.
