# Dependency updates

Dependabot checks Cargo manifests weekly using `increase-if-necessary`. It
leaves a requirement unchanged when that requirement already admits the new
release, avoiding PRs that only raise the minimum supported version.
`exclude-paths: ["**/Cargo.lock"]` excludes lockfiles from its input, so Cargo
dependency PRs update manifests only. Maintainers own lockfile refreshes and
must resolve and validate changed requirements before merging.

Cargo's Dependabot implementation does not support `widen`. With the existing
implicit caret requirements, an update such as `"1"` to `"2"` changes both
bounds. Strictly preserving the original minimum across major releases would
require explicit ranges such as `">=1, <2"`; the current policy preserves the
existing manifest syntax and does not claim compatibility across major versions.

The policy lives in `.github/dependabot.yml` and applies to the root and member
Cargo manifests. GitHub Actions updates retain their separate configuration.
This changes automation only and has no application runtime cost.

Validate configuration edits with
`prek run check-yaml --files .github/dependabot.yml` and `git diff --check`.
When reviewing a dependency PR, check that the new release falls outside the old
requirement and the PR contains no lockfile edits. After refreshing the
lockfile, run `make lint` and the tests relevant to the dependency change.

References: [Dependabot options][options] and the Cargo implementation's
[supported strategies][strategies], [file exclusions][fetcher], and [conditional
lockfile updates][updater].

[options]:
  https://docs.github.com/en/code-security/reference/supply-chain-security/dependabot-options-reference
[strategies]:
  https://github.com/dependabot/dependabot-core/blob/main/cargo/lib/dependabot/cargo/update_checker/requirements_updater.rb
[fetcher]:
  https://github.com/dependabot/dependabot-core/blob/main/cargo/lib/dependabot/cargo/file_fetcher.rb
[updater]:
  https://github.com/dependabot/dependabot-core/blob/main/cargo/lib/dependabot/cargo/file_updater.rb

## Frontend dependencies

Bun updates are configured separately for `/app` with `package-ecosystem: bun`.
Direct dependency requirements use explicit `>=minimum <next-major` ranges.
Scheduled version updates use `versioning-strategy: widen`: keep the existing
minimum and extend the upper bound only when a newer major falls outside the
current range. For example, `>=19.2.0 <20.0.0` can become `>=19.2.0 <21.0.0`; it
must not raise the `19.2.0` floor.

The job allows direct dependencies and ignores semver-minor and semver-patch
version updates. That prevents routine within-range and transitive-only lockfile
refresh PRs. An upper-bound update changes the manifest and regenerates
`app/bun.lock` together; the Cargo manifest-only policy above does not apply to
Bun. Maintainers can still make deliberate lockfile updates when needed.

This policy controls scheduled version-update PRs. Security-update behavior is
managed separately by GitHub; these settings do not disable the security update
service or promise that security fixes will never require lockfile changes.

GitHub supports the text `bun.lock`, not the legacy binary `bun.lockb`; see
[supported ecosystems](https://docs.github.com/en/code-security/reference/supply-chain-security/supported-ecosystems-and-repositories).
The Bun updater's `WidenRanges` implementation preserves the lower bound; see
the
[Bun requirements updater](https://github.com/dependabot/dependabot-core/blob/main/bun/lib/dependabot/bun/update_checker/requirements_updater.rb).

Validate frontend updates with `bun install --frozen-lockfile` from `app/`,
`make lint-ui`, `make test-ui`, and `make build-ui`. Run
`cargo test -p xtask --test release_workflow` when changing the policy;
regressions protect widening, direct-only updates, and minor/patch ignores.
Native dependencies and application release versions remain independent.
