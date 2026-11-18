---
name: bump-version
description: >-
  Update SpeleoDB Compass Sidecar to a user-specified X.Y.Z version in Cargo,
  refresh workspace versions in Cargo.lock, roll the changelog forward, and
  commit the release metadata together when explicitly requested. Use for
  requests such as "Bump the version to 26.9.22 using $bump-version." Skip
  tests, builds, and hooks for this release-metadata workflow.
---

# Bump SpeleoDB Compass Sidecar's version

Adapted from the mobile bump-version skill, following release commit
`c4e1c59fdcba930184a60a08e6f24191c6f16675`. Complete a small release-metadata
edit and, when explicitly requested, make **one commit containing the version
bump, lockfile, and changelog**.

## Input and scope

- Take the exact `X.Y.Z` version from the user's request. Do not choose the next
  version yourself. If it is missing, ask only for that value. Use valid SemVer
  without leading zeroes (for example, `26.9.2`, not `26.09.02`).
- Work from this repository's root. Read the version settings, workspace member
  manifests, and beginning of `CHANGELOG.md`. Inspect Git status and the
  relevant diffs to preserve existing work.
- This workflow edits release metadata. Follow `AGENTS.md`: staging and
  committing each require an explicit user request; invoking this skill alone
  does not authorize either action. It does not tag, push, publish, or trigger a
  release workflow unless separately requested.

## Keep the release edit lightweight

Use direct file edits and Git commands. The Cargo relock command below is the
only required generation step. Do not run tests, lint, formatters, builds,
validators, dependency audits, repository scripts, or review agents for this
routine release edit. Reading files and inspecting diffs are permitted.

Do not run `cargo bump-version` here: that alias compiles and runs `xtask` and
only edits the root Cargo version; it does not finish the lockfile/changelog
workflow. Disable commit hooks with the command-local setting below without
changing persistent Git configuration.

## 1. Update the Cargo version source

Set the requested version in:

| File         | Field                         |
| ------------ | ----------------------------- |
| `Cargo.toml` | `[workspace.package].version` |

Keep `app/src-tauri/tauri.conf.json` without a top-level `version`. Tauri uses
the inherited Cargo package version when that override is absent; do not add a
second version source during a release bump.

The private frontend `app/package.json` is unversioned and is not release
metadata.

Workspace packages inherit `version.workspace = true`; preserve that inheritance
instead of inserting versions into member manifests. Do not change dependency
requirements, app identifiers, signing settings, or the fixed
`SPELEODB_COMPASS_TOML_VERSION` metadata-schema marker.

## 2. Relock workspace package versions

After editing the root Cargo version, run from the repository root:

```bash
cargo update --workspace --offline
```

This refreshes the workspace packages while retaining locked third-party
dependencies. If required registry metadata is unavailable offline, retry
`cargo update --workspace` with the environment's required network permission.
If that fails, report the blocker and leave the release uncommitted.

Inspect the `Cargo.lock` diff: the expected changes are only the versions of
workspace packages inheriting the root version (currently `api`, `common`,
`speleodb-compass-sidecar` and `xtask`). Do not manually replace matching
version strings throughout the lockfile. Do not use plain `cargo update`,
`make update`, or regenerate the lockfile from scratch; these can upgrade
unrelated dependencies. Resolve unexpected dependency churn before committing,
preserving any pre-existing lockfile edits.

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

Only perform the Git actions explicitly requested by the user. Inspect the
intended diff and staged filenames first. When staging and committing are both
authorized and these files contain only this release's edits, run these direct
commands with the requested version:

```bash
git add -- Cargo.toml Cargo.lock CHANGELOG.md
git -c core.hooksPath=/dev/null commit --only -m "[compass_speleodb] Release - vX.Y.Z" -- Cargo.toml Cargo.lock CHANGELOG.md
```

Use the reference commit's message convention. The changelog and lockfile belong
in the same commit as the root Cargo version; do not create a separate changelog
commit. Explicit paths keep unrelated staged files out of the commit.

If unrelated edits share these files, do not use the whole-file commands above.
Commit only the release hunks while preserving unrelated working-tree and staged
changes. Never stage agent plans, notes, or scratch files. Amend only when the
user requests it, retaining all three release files and the release message.

## Completion message

Report the version and single commit ID, or state that the prepared changes
remain uncommitted when committing was not requested. Confirm that Cargo.lock
was refreshed and release notes moved below an empty `## [Unreleased]`. State
that tests and checks were skipped for this metadata workflow; do not claim
publication.
