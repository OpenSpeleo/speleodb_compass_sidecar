# Windows signing setup

The executable Bash scripts under `signing/windows/` provision a shared Azure
Artifact Signing account, a Public Trust certificate profile, and a passwordless
GitHub identity for each repository's `windows-signing` environment. They are
operator tools: the operator runs them locally with their own Azure and GitHub
authentication. They do not build or publish application releases. Reuse the
shared publisher account and profile for SpeleoDB Compass Sidecar. If this
repository already has working signing credentials and federation, no
provisioning step is needed for a release.

## Run locally

Install Azure CLI, GitHub CLI and Python 3. Authenticate with `az login` and
`gh auth login`, selecting the intended Azure subscription during login. The
Azure operator needs permission to create the resources, create Entra
applications and service principals, and assign Azure roles. The GitHub operator
needs permission to manage repository environments and environment secrets.

Set `SIGNING_ACCOUNT` at the top of `signing/windows/step1.sh` to the desired
globally unique account name (currently `usahi-signing`). The account name is an
identifier, not a secret. Then, from the repository root, run directly in zsh or
Bash:

```sh
./signing/windows/step1.sh
```

This uses the selected Azure subscription and configured signing account name,
installs the `artifact-signing` CLI extension when absent, and creates a
dedicated resource group and paid Basic account in East US. It grants the
logged-in user the identity-verification role. It does not grant the CI identity
resource-management permissions.

In Azure Portal, open the signing account, select **Identity validations**, and
create a **Public** validation. Complete the organization or individual checks
and wait for approval. This operation is available only in the portal, and
Public Trust eligibility depends on the publisher's legal identity and country.
Copy the approved identity validation ID, then run:

```sh
./signing/windows/step2.sh
./signing/windows/step3.sh
```

Step 2 prompts for that ID and creates the shared `usahi-public` Public Trust
profile. Step 3 creates or reuses the `USAHI Windows Signing` Entra application
and service principal, configures GitHub OIDC for the selected repository,
grants the certificate-profile signing role at the profile scope, and saves
these GitHub environment secrets through standard input:

- `AZURE_CLIENT_ID`
- `AZURE_TENANT_ID`
- `AZURE_SUBSCRIPTION_ID`
- `AZURE_SIGNING_ENDPOINT`
- `AZURE_SIGNING_ACCOUNT`
- `AZURE_SIGNING_PROFILE`

An existing `windows-signing` environment retains its protection rules. If it
does not exist, step 3 creates it without protection rules; configure those in
GitHub before a release. Its OIDC subject is
`repo:OpenSpeleo/speleodb_compass_sidecar:environment:windows-signing` for this
repository.

Step 3 selects the current GitHub repository using `gh repo view`. To select
this application explicitly while reusing the publisher setup, run:

```sh
SIGNING_REPO="OpenSpeleo/speleodb_compass_sidecar" ./signing/windows/step3.sh
```

Each repository gets a distinct federated credential, while sharing the same
account, approved publisher identity and certificate profile. Adding a
repository preserves existing repositories' authorization. Azure limits the
number of federated credentials per application; use another CI application if
that limit is reached, without creating another signing account.

New setups use the `usahi-signing` resource group and account, and the
`usahi-public` certificate profile. Saved resource-group names take precedence
over defaults; editing defaults does not move Azure resources. After deleting a
resource group, clear its cached account, endpoint, validation and profile IDs
in the private state before restarting step 1, then repeat identity validation
and steps 2 and 3.

## Private state and reruns

Each step runs in its own Bash process through its shebang; do not source the
scripts or paste their contents into zsh. Bash's interactive `read -p` option
has a different meaning in zsh.

Configuration is saved to
`${XDG_CONFIG_HOME:-$HOME/.config}/usahi/windows-signing/state.sh`, outside
every Git checkout. The directory has mode `700` and the file mode `600`.
Identifier prompts display what you type; hidden input is reserved for secrets
or private information. Scripts disable shell tracing. The file contains
Bash-escaped assignments and is sourced by the scripts; treat it as trusted
private local configuration. It contains identifiers and provisioning progress;
no client password, private key, or certificate export is generated.

Steps reuse the stored subscription/account, existing profiles, service
principal, federated credential and role assignments. A listing or permission
failure stops the step rather than being treated as an absent resource. After
fixing authentication, role propagation or a failed CLI operation, rerun the
same step. To correct the saved account name, edit `SIGNING_ACCOUNT` at the top
of `step1.sh` and rerun:

```sh
./signing/windows/step1.sh
```

The override replaces the saved name and clears the previous account's cached
resource, endpoint, validation and profile IDs. It preserves the subscription
and shared CI application identity and does not rename or delete any Azure
resource. If an account with the corrected name is absent, step 1 creates it as
usual; complete steps 2 and 3 for that account afterward.

Finish a running setup step before editing its script. Step 2 loads its full
provisioning flow before execution. If profile creation completed but the local
step was interrupted, rerunning step 2 reuses the existing Public Trust profile
and saves its resource ID.

Azure CLI flattens resource properties in its JSON output: the scripts read
`accountUri` and `profileType` at the top level, rather than using the REST
API's nested `properties` object. An interrupted CLI wait can leave an account
created in Azure; step 1 reuses that account when rerun.

Keep the private state file to retain application identity across reruns.
Creation and local state saving are not an Azure transaction; if an application
creation succeeds but its response or local state write fails, reconcile that
application in Entra before retrying to avoid an orphan.

Publisher identity appears publicly in code-signing certificates by design.
Optional street and postal fields are omitted. Do not paste state files, Azure
error details containing personal data, or credential values into issues.

## Release integration and verification

These scripts prepare Azure and GitHub. The release workflow grants
`id-token: write` and uses `azure/login` to authenticate with the environment's
federated identity. A pinned Microsoft ArtifactSigning PowerShell module and its
downloaded dependencies are cached between runs; the version is declared in the
release workflow. No client secret or exported signing certificate is needed.

A temporary CI configuration supplies Tauri's `bundle.windows.signCommand`. The
small `scripts/release/windows-sign.ps1` adapter permits only Azure CLI
authentication, signs with a timestamp, and checks the resulting Authenticode
signature before returning success. Tauri signs the application, installers,
uninstaller and any unsigned bundled executable/DLL resources. Existing valid
vendor resource signatures are preserved. Local builds retain their existing
unsigned Windows configuration.

The signing adapter checks the service access token at each invocation. When the
earlier login's GitHub assertion has expired, it requests a fresh assertion and
renews the Azure CLI session using the same environment identity. This supports
long compilations and signing runs without logging in for every file or adding
client secrets. Token acquisition is retried once after renewal; authentication
failures stop the release before signing or uploading assets.

Signing happens during bundling, before Tauri generates updater signatures and
`tauri-action` uploads the draft assets. The existing updater signing keys are
separate and unchanged. Never re-sign installers after updater signatures have
been generated. See
[release signing architecture](release-workflow.md#code-signing).

Check setup scripts with `bash -n` and validate documentation with the
repository hooks. Release regression tests cover signing configuration and the
callback; see [release checks](release-workflow.md#regression-checks). Offline
checks cannot establish Azure permissions, identity approval, regional
availability or successful Authenticode signatures. The hosted Windows job must
produce valid timestamped signatures and pass updater verification. Operator
setup adds no application runtime cost; signing runs during release packaging,
with its tools and dependencies cached between builds.

The authoritative setup and integration references are Microsoft's
[Artifact Signing quickstart](https://learn.microsoft.com/en-us/azure/artifact-signing/quickstart),
the
[certificate-profile CLI reference](https://learn.microsoft.com/en-us/cli/azure/artifact-signing/certificate-profile),
Microsoft's
[OIDC integration](https://github.com/Azure/artifact-signing-action/blob/main/docs/OIDC.md),
and Tauri's
[Windows signing guide](https://v2.tauri.app/distribute/sign/windows/).
