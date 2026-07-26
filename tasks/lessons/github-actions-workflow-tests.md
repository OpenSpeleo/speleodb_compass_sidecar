# GitHub Actions workflow-test diagnostics

- Before attributing a CI-only workflow assertion to stale checkout or cache
  state, compare the runner's `headSha` with the repository's live remote ref
  and inspect the files stored in that exact commit.
- Tests that return byte slices from line-oriented text must calculate offsets
  from the original newline-inclusive chunks. `str::lines()` removes line
  terminators, so adding one byte per line is incorrect for CRLF checkouts.
- Add explicit LF and CRLF cases for helpers that parse committed text files and
  run on both Unix and Windows.
