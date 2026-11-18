#!/usr/bin/env bash
# Validate an App Store Connect team API key and store it in the macOS Keychain.
# shellcheck source-path=SCRIPTDIR
# shellcheck source=common.sh
source "$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)/common.sh"

main() {
    require xcrun
    [[ -n ${SIGNING_CERTIFICATE_PATH:-} ]] || fail 'Run step1.sh first.'
    printf '%s\n' 'Use an App Store Connect team API key: Users and Access > Integrations > App Store Connect API > Team Keys.'
    prompt_value SIGNING_API_KEY_PATH 'Path to the downloaded .p8 file'
    SIGNING_API_KEY_PATH=$(private_file "$SIGNING_API_KEY_PATH")
    prompt_value SIGNING_API_KEY_ID 'Key ID'
    prompt_value SIGNING_API_ISSUER 'Issuer ID (above the team keys table)'
    prompt_value SIGNING_KEYCHAIN_PROFILE 'Local notarization Keychain profile name'
    xcrun notarytool store-credentials "$SIGNING_KEYCHAIN_PROFILE" \
        --key "$SIGNING_API_KEY_PATH" --key-id "$SIGNING_API_KEY_ID" --issuer "$SIGNING_API_ISSUER"
    save_state
    printf 'Notarization credentials validated and saved. Run ./signing/macos/step3.sh.\n'
}

main "$@"; exit "$?"
