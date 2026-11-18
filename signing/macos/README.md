# 1. GitHub Environment Setup

These credentials belong to SpeleoDB Compass Sidecar. Reuse the existing
publisher certificate and API key; there is no need to recreate working
credentials for a release. The numbered scripts support setup and rotation; see
[macOS signing setup](../../docs/macos-signing-setup.md) and the
[release workflow](../../docs/release-workflow.md). Keep private files outside
every Git checkout.

First, create a GitHub environment for the macOS credentials:

1. Open `OpenSpeleo/speleodb_compass_sidecar` on GitHub.
2. Go to `Settings → Environments`.
3. Click `New environment`.
4. Name it `macos-signing`.
5. Click `Configure environment`.

# 2. `APPLE_CERTIFICATE` setup

The first secret will be `APPLE_CERTIFICATE`. We need to export your signing
certificate and private key first:

1. Open `Keychain Access` → `login` → `My Certificates`.
2. Find the `Developer ID Application` certificate used to sign the application.
3. Right-click it and select Export.
4. Choose Personal Information Exchange (.p12).
5. Save it as DeveloperID.p12.
6. Set a strong export password and save that password in your password manager.

The exported certificate must include its private key—the key shown underneath
it when expanded in Keychain Access.

Copy the certificate as Base64 to your clipboard:

`openssl base64 -A -in "DeveloperID.p12" | pbcopy`

Then in `Settings` → `Environments` → `macos-signing` → Add environment secret:

- Name: `APPLE_CERTIFICATE`
- Value: paste from your clipboard.

# 3. `APPLE_CERTIFICATE_PASSWORD` setup

Add the next environment secret in macos-signing:

- Name: APPLE_CERTIFICATE_PASSWORD
- Value: the password you chose when exporting the .p12 file.

# 4. `APPLE_SIGNING_IDENTITY` setup

```bash
security find-identity -v -p codesigning |
  sed -n 's/.*"\(Developer ID Application:.*\)".*/\1/p' |
  pbcopy
```

In macos-signing, add:

- Name: `APPLE_SIGNING_IDENTITY`
- Value: paste from your clipboard.

# 5. `APPLE_API_KEY` setup

- Name: `APPLE_API_KEY`
- Value: the Key ID from `App Store Connect` → `Users and Access` →
  `Integrations` → `App Store Connect API` → `Team Keys`.

# 6. `APPLE_API_ISSUER` setup

- Name: `APPLE_API_ISSUER`
- Value: the Issuer ID shown above the Team Keys list in App Store Connect—the
  issuer associated with the team API key used for Sidecar notarization.

# 7. `APPLE_API_PRIVATE_KEY` setup

```sh
pbcopy < "/path/outside-checkout/AuthKey_YOUR_KEY_ID.p8"
```

In macos-signing, add:

- Name: `APPLE_API_PRIVATE_KEY`
- Value: paste the complete contents of your `.p8` file, including its header
  and footer.
