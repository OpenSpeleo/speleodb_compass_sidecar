# macOS signing setup

The numbered scripts in `signing/macos/` prepare a Developer ID signing
identity, validate notarization credentials, and configure the existing
`macos-signing` GitHub environment. They follow the Windows setup's local-state
model and can configure multiple repositories belonging to the same publisher.
They are operator tools, separate from the release build and updater signing.
Reuse the existing publisher identity and API key for SpeleoDB Compass Sidecar;
setup is needed only when configuring or rotating credentials, not for each
release.

## Prerequisites and private files

Use macOS with Xcode's command-line tools, Python 3 and GitHub CLI. Authenticate
with `gh auth login`. You need an Apple Developer Program membership, permission
to manage the team's certificates and API keys, and permission to manage the
repository's GitHub environment secrets.

Keep credential files outside every Git checkout. A suitable location is
`~/.config/usahi/macos-signing/credentials/`, with directory permissions `700`
and file permissions `600`. Keep an encrypted backup of the `.p12`, its export
password and the downloaded `.p8` key. GitHub secrets cannot be downloaded later
to restore these files.

The scripts save paths and identifiers to
`${XDG_CONFIG_HOME:-$HOME/.config}/usahi/macos-signing/state.sh`, using
directory mode `700` and file mode `600`. This is trusted, Bash-escaped local
configuration; do not commit it. The `.p12` password is requested only when
uploading to GitHub, is hidden while typing, and is never saved in this state
file. Paths, identity names, Key IDs and Issuer IDs remain visible. The scripts
refer to your original credential files, so retain those files at the saved
paths.

## Step 1: Developer ID Application certificate

If you already have the working certificate and its private key in Keychain
Access, export that identity; you do not need a new certificate for each app. If
the old private key is lost, a downloaded `.cer` alone cannot recover it. Create
a fresh identity:

1. Open Keychain Access and use **Certificate Assistant → Request a Certificate
   From a Certificate Authority**. Enter your email and common name, select
   **Saved to disk**, and save the certificate signing request. Keep the
   corresponding private key in this Mac's Keychain.
2. In the Apple Developer account's **Certificates, Identifiers & Profiles →
   Certificates**, add a **Developer ID Application** certificate and upload the
   request. The Account Holder can create this certificate.
3. Download the `.cer` and double-click it to install it on the same Mac.
4. In Keychain Access's **My Certificates**, select the Developer ID Application
   identity with its associated private key. Export it as a password-protected
   `.p12` file outside the checkout. Save the export password in your password
   manager. This is the export password, not your Apple account password.

Apple documents
[certificate signing requests](https://developer.apple.com/help/account/certificates/create-a-certificate-signing-request/),
[Developer ID certificate creation](https://developer.apple.com/help/account/certificates/create-developer-id-certificates/)
and
[exporting signing identities](https://developer.apple.com/documentation/xcode/sharing-your-teams-signing-certificates).

From the repository root, run:

```sh
bash signing/macos/step1.sh
```

Enter the `.p12` path and the full `Developer ID Application: … (TEAMID)`
identity shown by the script. Use the identity belonging to that export. The
script checks that the identity is valid in your local Keychain and saves the
configuration. It does not export unrelated keys or change your Keychain's
access policy.

## Step 2: notarization API key and Keychain profile

Use an **App Store Connect team API key**, matching the existing CI contract. In
[App Store Connect](https://appstoreconnect.apple.com/access/integrations/api),
open **Users and Access → Integrations → App Store Connect API → Team Keys**. If
API access is unavailable, the Account Holder must request access first.
Generate a key with Developer access and download its `.p8` file. Apple permits
that download only once. An existing working team key can be reused.

The **Key ID** is in the key's table row. The **Issuer ID** is above the team
keys table; it is not the Apple Developer Team ID. Individual API keys do not
use an Issuer ID and are not the credential type used by these scripts or this
CI. See Apple's
[API access guide](https://developer.apple.com/help/app-store-connect/get-started/app-store-connect-api/)
and Tauri's
[macOS notarization guide](https://v2.tauri.app/distribute/sign/macos/#notarization).

```sh
bash signing/macos/step2.sh
```

Enter the `.p8` path, Key ID, Issuer ID and a local Keychain profile name. The
default is `usahi-notarization`; you may enter an existing profile name to
update it. `notarytool store-credentials` validates with Apple before saving
credentials in Keychain. A failed validation stops the script without saving new
setup values. Keychain may request macOS authorization.

## Step 3: GitHub environment

```sh
bash signing/macos/step3.sh
```

The script selects the current repository using `gh repo view`, displays the
destination, and requests the `.p12` export password with hidden input. It
checks that the export contains the selected valid Keychain certificate and its
matching private key before writing GitHub secrets. Validation uses macOS
OpenSSL to support Keychain export encryption; decrypted key material stays in
pipes.

It preserves an existing environment's deployment protection rules. If it must
create `macos-signing`, configure the new environment's protection rules in
GitHub before releasing. All secret values travel through standard input:

| Environment secret           | Source                                        |
| ---------------------------- | --------------------------------------------- |
| `APPLE_CERTIFICATE`          | Base64 encoding of the complete `.p12` file   |
| `APPLE_CERTIFICATE_PASSWORD` | Hidden password prompt                        |
| `APPLE_SIGNING_IDENTITY`     | Developer ID Application identity from step 1 |
| `APPLE_API_KEY`              | Team API Key ID                               |
| `APPLE_API_ISSUER`           | Team API Issuer ID                            |
| `APPLE_API_PRIVATE_KEY`      | Complete original `.p8` contents              |

`APPLE_API_KEY_PATH` is a temporary path created by CI, not another secret. The
updater's `TAURI_SIGNING_*` repository secrets are separate and unchanged. To
select this repository explicitly when reusing the existing publisher setup:

```sh
SIGNING_REPO="OpenSpeleo/speleodb_compass_sidecar" bash signing/macos/step3.sh
```

Steps 1 and 2 display saved values as defaults; press Enter to reuse them or
type replacements. Rerun step 3 after rotating a certificate or API key. A
failed GitHub upload can leave only some secrets updated; rerun it to write the
complete set. Finish any running step before editing scripts.

## Manual DMG notarization and delayed reviews

The existing release workflow owns signing and packaging, including embedded
application resources. Tauri handles app notarization and stapling; the workflow
separately submits and staples the final DMG. See
[release signing architecture](release-workflow.md#code-signing). Setup scripts
do not rebuild, re-sign or publish artifacts.

For an already Developer ID-signed DMG containing a correctly signed app, submit
without waiting for review. Substitute the actual DMG path and, if changed, your
Keychain profile name:

```sh
xcrun notarytool submit "/path/to/app.dmg" --keychain-profile usahi-notarization
```

Save the returned submission ID. Upload success does not mean acceptance. Check
that submission without uploading again:

```sh
xcrun notarytool info "SUBMISSION_ID" --keychain-profile usahi-notarization
```

To wait locally with a bounded timeout:

```sh
xcrun notarytool wait "SUBMISSION_ID" --keychain-profile usahi-notarization --timeout 30m
```

Apple continues processing after a local wait times out. If rejected, retrieve
the report using
`xcrun notarytool log "SUBMISSION_ID" --keychain-profile usahi-notarization "/path/outside-checkout/notarization-log.json"`
and correct the reported signing issue. Logs may contain identifying paths;
review before sharing.

Only after **Accepted**, staple and validate the same unchanged DMG:

```sh
xcrun stapler staple "/path/to/app.dmg"
xcrun stapler validate "/path/to/app.dmg"
```

Repeat per artifact. A ticket on the DMG does not attach a ticket to the app
inside it. The normal release workflow staples the app before packaging it and
checks both. Avoid modifying or re-signing completed release assets afterward,
which can invalidate existing signatures or updater metadata. Initial acceptance
does not exempt future builds from notarization or guarantee shorter reviews.
Apple's
[custom notarization workflow](https://developer.apple.com/documentation/security/customizing-the-notarization-workflow)
describes submission, status, reports and stapling.

## Verification and scope

Check setup scripts with `bash -n` and validate documentation with the
repository hooks. Release regression tests cover the workflow and artifact
verification; see [release checks](release-workflow.md#regression-checks). Local
checks do not prove Apple account eligibility or a successful hosted release.
The hosted macOS job must validate the signed app and the stapled DMG before
replacing the draft asset. Operator setup adds no application runtime cost;
notarization waits and artifact validation occur only during release packaging.
