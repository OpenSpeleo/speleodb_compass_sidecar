#!/usr/bin/env bash
# Send the six Apple credentials expected by the release workflow to GitHub.
# shellcheck source-path=SCRIPTDIR
# shellcheck source=common.sh
source "$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)/common.sh"

save_secret() {
    printf '%s' "$2" | gh secret set "$1" --repo "$SIGNING_REPO" --env macos-signing >/dev/null
}

validate_certificate() {
    local certificate fingerprint identities certificate_key exported_key
    # Use macOS OpenSSL, which supports Keychain's PKCS#12 export encryption.
    certificate=$(printf '%s\n' "$certificate_password" | /usr/bin/openssl pkcs12 \
        -in "$SIGNING_CERTIFICATE_PATH" -passin stdin -clcerts -nokeys)
    fingerprint=$(printf '%s\n' "$certificate" | /usr/bin/openssl x509 -noout -fingerprint -sha1)
    fingerprint=${fingerprint#*=}
    fingerprint=${fingerprint//:/}
    identities=$(security find-identity -v -p codesigning)
    printf '%s\n' "$identities" | python3 -c '
import re, sys
identities = re.findall(r"([0-9A-Fa-f]{40})\s+\"([^\"]+)\"", sys.stdin.read())
if not any(key.upper() == sys.argv[1].upper() and name == sys.argv[2] for key, name in identities):
    sys.exit("The export does not contain the selected valid Keychain identity. Export that identity again.")
' "$fingerprint" "$SIGNING_IDENTITY"
    certificate_key=$(printf '%s\n' "$certificate" | /usr/bin/openssl x509 -pubkey -noout | \
        /usr/bin/openssl pkey -pubin -outform DER | shasum -a 256)
    # Decrypted private key material passes only through a pipe, never a file.
    exported_key=$(printf '%s\n' "$certificate_password" | /usr/bin/openssl pkcs12 \
        -in "$SIGNING_CERTIFICATE_PATH" -passin stdin -nocerts -nodes | \
        /usr/bin/openssl pkey -pubout -outform DER | shasum -a 256)
    [[ "$certificate_key" == "$exported_key" ]] || fail 'The export must include the matching private key.'
}

main() {
    require gh
    require security
    require shasum
    require_setup
    gh auth status >/dev/null 2>&1 || fail 'Authenticate with gh auth login first.'
    SIGNING_REPO=${SIGNING_REPO:-$(gh repo view --json nameWithOwner --jq .nameWithOwner)}
    [[ "$SIGNING_REPO" =~ ^[A-Za-z0-9_.-]+/[A-Za-z0-9_.-]+$ ]] || fail 'Expected SIGNING_REPO in owner/repository form.'
    printf 'Configuring %s environment macos-signing.\n' "$SIGNING_REPO"

    local certificate_password environments
    read -r -s -p 'Certificate export password: ' certificate_password || fail 'Input canceled.'
    printf '\n' >&2
    [[ -n "$certificate_password" ]] || fail 'Use a password-protected .p12 export.'
    validate_certificate

    # Preserve deployment protection rules; a listing failure must stop setup.
    environments=$(gh api --paginate "repos/$SIGNING_REPO/environments" --jq '.environments[].name')
    if ! printf '%s\n' "$environments" | python3 -c '
import sys
sys.exit(0 if any(line.strip().casefold() == "macos-signing" for line in sys.stdin) else 1)
'; then
        gh api --method PUT "repos/$SIGNING_REPO/environments/macos-signing" >/dev/null
        printf 'Created macos-signing. Configure its deployment protection rules in GitHub.\n'
    fi

    # Stream private file contents; never put passwords or keys in command arguments.
    python3 - "$SIGNING_CERTIFICATE_PATH" <<'PY' | gh secret set APPLE_CERTIFICATE --repo "$SIGNING_REPO" --env macos-signing >/dev/null
import base64, sys
from pathlib import Path
sys.stdout.write(base64.b64encode(Path(sys.argv[1]).read_bytes()).decode('ascii'))
PY
    save_secret APPLE_CERTIFICATE_PASSWORD "$certificate_password"
    unset certificate_password
    save_secret APPLE_SIGNING_IDENTITY "$SIGNING_IDENTITY"
    save_secret APPLE_API_KEY "$SIGNING_API_KEY_ID"
    save_secret APPLE_API_ISSUER "$SIGNING_API_ISSUER"
    gh secret set APPLE_API_PRIVATE_KEY --repo "$SIGNING_REPO" --env macos-signing < "$SIGNING_API_KEY_PATH" >/dev/null
    printf 'All six Apple secrets saved to macos-signing. Local configuration remains available for reruns.\n'
}

main "$@"; exit "$?"
