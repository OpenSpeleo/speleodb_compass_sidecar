# Windows release bootstrap shell

## Diagnosis

- [x] Trace the Windows release failure to Bash syntax being parsed by the
      runner's default PowerShell shell.
- [x] Confirm that the same workflow step runs on both macOS and Windows.

## Implementation

- [x] Add regression coverage requiring the Trunk bootstrap to select Bash
      explicitly.
- [x] Set the Trunk installation step's shell to Bash without changing its
      authentication or locked fallback behavior.
- [x] Record the release-tooling fix in the changelog and release-workflow
      documentation.

## Verification

- [x] Run the focused release-workflow regression test before and after the
      workflow fix.
- [x] Run the complete `xtask` test suite.
- [x] Validate formatting, lint, workflow YAML, whitespace, and final
      staged/unstaged state.

## Review

The focused regression test failed before the workflow change with
`the cross-platform Trunk bootstrap must run its shell script with Bash`. After
adding `shell: bash` to the `Install trunk` step, the assertion passed while the
existing token and locked-command assertions continued to pass.

Final verification:

- `cargo test -p xtask --test release_workflow`: 4 passed
- `cargo test -p xtask`: 11 passed
- `prek run check-yaml --files .github/workflows/publish.yml`: passed
- `cargo fmt --all -- --check`: passed
- `make lint`: passed
- `git diff --check`: passed

The workspace began with no staged, unstaged, or untracked changes. The final
changes are all task-related and remain unstaged.
