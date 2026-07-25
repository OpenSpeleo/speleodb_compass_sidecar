# Release tool bootstrap reliability

## Diagnosis

- [x] Identify the historical release failure as an unauthenticated
  `cargo-binstall` lookup that received a GitHub API 403 and fell back to an
  unlocked source build of Trunk.
- [x] Confirm that the unlocked build selected incompatible `lightningcss`,
  `parcel_selectors`, and `cssparser` versions.
- [x] Confirm that later commits corrected the obsolete Tauri/cache paths and
  added `--locked`, but the publish workflow still leaves the Trunk install
  step unauthenticated.

## Implementation

- [x] Add a local regression test for the release workflow.
- [x] Run the test before the fix and confirm it reproduces the missing-token
  failure.
- [x] Pass `GITHUB_TOKEN` to the Trunk installation step while retaining the
  locked fallback and corrected repository paths.
- [x] Document the failure mode, release-tool safeguards, and local regression
  command.
- [x] Update contributor instructions to preserve the release safeguards.

## Verification

- [x] Run the focused release-workflow regression test.
- [x] Run the full `xtask` test suite.
- [x] Validate the workflow YAML.
- [x] Run Rust formatting and lint checks.
- [x] Run whitespace checks and inspect final staged/unstaged state.

## Review

The regression test failed before the workflow change because the `Install
trunk` step did not contain `GITHUB_TOKEN`; its locked-command and repository
path assertions already passed. After adding a step-scoped token, both focused
tests passed.

Final verification:

- `cargo test -p xtask --test release_workflow`: 2 passed
- `cargo test -p xtask`: 9 passed
- `prek run check-yaml --files .github/workflows/publish.yml`: passed
- `cargo fmt --all -- --check`: passed
- `make lint`: passed
- `git diff --check`: passed

The workspace began with no staged, unstaged, or untracked changes. The final
changes are all task-related and remain unstaged.
