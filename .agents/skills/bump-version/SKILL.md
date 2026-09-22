---
name: bump-version
description: >-
  Update SpeleoDB Compass Sidecar to a user-specified X.Y.Z version in Cargo and
  Tauri, refresh workspace versions in Cargo.lock, roll the changelog forward,
  and commit the release metadata together. Use for requests such as "Bump the
  version to 26.9.22 using $bump-version." Skip tests, builds, and hooks for
  this release-metadata workflow.
---

# Bump SpeleoDB Compass Sidecar's version

Adapted from the mobile bump-version skill, following release commit
`c4e1c59fdcba930184a60a08e6f24191c6f16675`. Complete a small release-metadata
edit and make **one commit containing the version bump, lockfile, and
changelog**.

## Input and scope

- Take the exact `X.Y.Z` version from the user's request. Do not choose the next
  version yourself. If it is missing, ask only for that value. Use valid SemVer
  without leading zeroes (for example, `26.9.2`, not `26.09.02`).
- Work from this repository's root. Read the version settings, workspace member
  manifests, and beginning of `CHANGELOG.md`. Inspect Git status and the
  relevant diffs to preserve existing work.
- This workflow edits and commits release metadata. It does not tag, push,
  publish, or trigger a release workflow unless separately requested.

## Keep the release edit lightweight

Use direct file edits and Git commands. The Cargo relock command below is the
only required generation step. Do not run tests, lint, formatters, builds,
validators, dependency audits, repository scripts, or review agents for this
routine release edit. Reading files and inspecting diffs are permitted.

Do not run `cargo bump-version` here: that alias compiles and runs `xtask` and
only edits the two version sources; it does not finish the lockfile/changelog
workflow. Disable commit hooks with the command-local setting below without
changing persistent Git configuration.

## 1. Update both version sources

Set the same version in:

| File                            | Field                         |
| ------------------------------- | ----------------------------- |
| `Cargo.toml`                    | `[workspace.package].version` |
| `app/src-tauri/tauri.conf.json` | Top-level `version`           |

Workspace packages inherit `version.workspace = true`; preserve that inheritance
instead of inserting versions into member manifests. Do not change dependency
requirements, app identifiers, signing settings, or the fixed
`SPELEODB_COMPASS_TOML_VERSION` metadata-schema marker.

## 2. Relock workspace package versions

After editing both version sources, run from the repository root:

```bash
cargo update --workspace --offline
```

This refreshes the workspace packages while retaining locked third-party
dependencies. If required registry metadata is unavailable offline, retry
`cargo update --workspace` with the environment's required network permission.
If that fails, report the blocker and leave the release uncommitted.

Inspect the `Cargo.lock` diff: the expected changes are only the versions of
workspace packages inheriting the root version (currently `api`, `common`,
`speleodb-compass-sidecar`, `speleodb-compass-sidecar-ui`, and `xtask`). Do not
manually replace matching version strings throughout the lockfile. Do not use
plain `cargo update`, `make update`, or regenerate the lockfile from scratch;
these can upgrade unrelated dependencies. Resolve unexpected dependency churn
before committing, preserving any pre-existing lockfile edits.

## 3. Roll the changelog forward

Move the existing Unreleased notes into a new release section, leaving a fresh,
empty Unreleased section at the top:

```markdown
## [Unreleased]

## vX.Y.Z

### Features

- Existing release notes...
```

Follow this repository's headings exactly: bracketed `[Unreleased]`, then
`vX.Y.Z` without an appended date. Preserve all former Unreleased notes and
their populated categories, including tooling, tests, and documentation.
Preserve implementation commit references when present. Do not invent release
notes or add an entry merely announcing the version bump. Omit empty categories
in the released section and leave older releases intact.

Do not copy the mobile skill's empty category scaffold. If this release is
already prepared, reuse its heading rather than duplicating it. A completed
release with no pending changes needs no new commit.

## 4. Commit the release metadata together

Inspect the intended diff and staged filenames. For files containing only this
release's edits, run these direct commands with the requested version:

```bash
git add -- Cargo.toml app/src-tauri/tauri.conf.json Cargo.lock CHANGELOG.md
git -c core.hooksPath=/dev/null commit --only -m "[compass_speleodb] Release - vX.Y.Z" -- Cargo.toml app/src-tauri/tauri.conf.json Cargo.lock CHANGELOG.md
```

Use the reference commit's message convention. The changelog and lockfile belong
in the same commit as both version sources; do not create a separate changelog
commit. Explicit paths keep unrelated staged files out of the commit.

If unrelated edits share these files, do not use the whole-file commands above.
Commit only the release hunks while preserving unrelated working-tree and staged
changes. Never stage agent plans, notes, or scratch files. Amend only when the
user requests it, retaining all four release files and the release message.

## Completion message

Report the version and single commit ID. Confirm that Cargo.lock was refreshed
and release notes moved below an empty `## [Unreleased]`. State that tests and
checks were skipped for this metadata workflow; do not claim publication.
