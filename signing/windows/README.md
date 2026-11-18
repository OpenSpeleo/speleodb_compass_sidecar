# Windows signing setup

These scripts configure SpeleoDB Compass Sidecar to use the existing shared
Azure Artifact Signing publisher account and Public Trust profile. The release
workflow consumes the `windows-signing` environment; it does not run these
operator setup scripts.

See [Windows signing setup](../../docs/windows-signing-setup.md) for
prerequisites, the numbered setup steps, repository-specific GitHub OIDC
federation, private state and credential rotation. See the
[release workflow](../../docs/release-workflow.md) for signing, artifact
verification and updater metadata publication.

Keep the shared `usahi-*` resource names and local state when reusing the
publisher setup. Do not create new paid resources merely to add this repository.
If signing is already configured, no setup step is needed for each release.
