# Cross-platform workflow regression tests

## Diagnosis

- [x] Compare the failing runner SHA with the standalone repository's remote
      `master` SHA.
- [x] Confirm the committed publish workflow contains the expected
      `GITHUB_TOKEN` declaration.
- [x] Reproduce the false failure by evaluating the workflow helper with CRLF
      line endings.

## Implementation

- [x] Make named-step extraction preserve exact newline byte offsets.
- [x] Add regression coverage for LF and CRLF workflow content.
- [x] Document the cross-platform test-harness fix in the changelog.
- [x] Record the corrected diagnostic approach in `tasks/lessons/`.

## Verification

- [x] Run the focused workflow regression tests.
- [x] Run the complete `xtask` test suite.
- [x] Run Rust formatting and lint checks.
- [x] Run whitespace checks and inspect staged and unstaged state separately.

## Review

The failing Windows runner checked out the current standalone `master` commit
`0692afe3`, and that commit's publish workflow already contained the expected
step-scoped `GITHUB_TOKEN`. The failure was therefore not caused by a stale
checkout or cache.

The regression helper used `str::lines()` and reconstructed byte offsets by
adding one byte per line. Git's Windows checkout used CRLF endings, so every
line actually occupied two terminator bytes and the extracted step drifted
backward into unrelated workflow content. The helper now scans newline-inclusive
chunks and uses their exact byte lengths. A synthetic regression verifies both
LF and CRLF input and confirms the following step is excluded.

Final verification:

- Remote `master`: `0692afe38cccb4e69b0b9e45cf5d7c8243cf0368`, matching the CI log
- CRLF reproduction before the fix: token assertion false
- `cargo test -p xtask --test release_workflow`: 4 passed
- `cargo test -p xtask`: 11 passed
- `make lint`: passed
- `cargo fmt --all -- --check`: passed
- `git diff --check` and `git diff --cached --check`: passed

Pre-existing staged WASM-cache changes remain staged. This task's changelog,
CRLF helper/test, task record, and lesson remain unstaged.
