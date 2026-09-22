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
