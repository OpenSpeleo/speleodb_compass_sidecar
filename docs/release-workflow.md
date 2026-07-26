# Release workflow

The release workflow is defined in `.github/workflows/publish.yml` and is
started manually with `workflow_dispatch`. It builds the Tauri application for
Apple Silicon macOS and Windows, then creates a draft GitHub release.

## Release tool bootstrap safeguards

The workflow installs Trunk with `cargo-binstall` before running `tauri-action`.
Preserve all of these safeguards:

- The cross-platform `Install trunk` script explicitly uses `shell: bash`.
  Windows runners default `run` steps to PowerShell, which cannot parse the
  script's POSIX `if ! command -v ...; then` guard.
- The `Install trunk` step receives `GITHUB_TOKEN`. `cargo-binstall` queries the
  GitHub API while looking for a prebuilt Trunk archive, and unauthenticated
  requests can be rate-limited.
- The guarded install command uses `--force`. Cargo caches can restore
  `.crates2.json` claiming Trunk is installed without restoring the executable;
  forcing the install when `command -v trunk` fails repairs that inconsistent
  state.
- The install command uses `--locked`. If a prebuilt archive is unavailable,
  `cargo-binstall` falls back to `cargo install`; the lock flag makes that
  source build use Trunk's published `Cargo.lock` instead of resolving a new,
  potentially incompatible dependency graph.
- Rust caching uses the repository root without a `src-tauri` workspace
  override. The Tauri project is under `app/src-tauri`.
- The Trunk cache uses `app/dist`, which is the UI build output directory.

These protections are covered by a dependency-free integration test:

```bash
cargo test -p xtask --test release_workflow
```

Run that test whenever the publish workflow, tool installation, or application
paths change.

## Historical failure

In release `v26.6.10`, the prebuilt-artifact lookup received HTTP 403 responses
from the GitHub API. `cargo-binstall` then compiled Trunk 0.21.14 from source
without its lockfile. The fresh resolution combined `lightningcss`
1.0.0-alpha.65 with incompatible `cssparser` versions selected through
`parcel_selectors`, causing 61 Rust type and trait errors.

The same log also reported that a top-level `src-tauri` directory did not exist.
That message came from a stale cache workspace and was not the fatal error, but
the cache configuration has since been corrected.

If this failure returns, check the `Install trunk` step first:

1. A PowerShell parser error at `if ! command -v trunk` means the step lost its
   explicit `shell: bash`.
2. A 403 during artifact discovery usually means the step is missing
   `GITHUB_TOKEN` or the token was not made available to the job.
3. `cargo-binstall` reporting Trunk as installed followed by
   `trunk: command not found` means the guarded repair command lost `--force`.
4. A long source compilation followed by duplicate-`cssparser` errors usually
   means `--locked` was removed or ignored.
5. A missing `src-tauri` message means a cache action is using the obsolete
   top-level path instead of `app/src-tauri`.

Do not patch `lightningcss` source in the release runner. Correct the tool
bootstrap inputs so the release is deterministic.
