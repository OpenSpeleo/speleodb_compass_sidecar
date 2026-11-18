#!/usr/bin/env bash
# Register an exported Developer ID Application identity for local and CI use.
# shellcheck source-path=SCRIPTDIR
# shellcheck source=common.sh
source "$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)/common.sh"

main() {
    require security
    printf '%s\n' 'Export your Developer ID Application certificate and private key as a password-protected .p12.' \
        'See docs/macos-signing-setup.md for fresh certificate creation and export.'
    prompt_value SIGNING_CERTIFICATE_PATH 'Path to the exported .p12 file'
    SIGNING_CERTIFICATE_PATH=$(private_file "$SIGNING_CERTIFICATE_PATH")
    local identities
    identities=$(security find-identity -v -p codesigning)
    printf '%s\n' "$identities"
    prompt_value SIGNING_IDENTITY 'Full Developer ID Application identity (without surrounding quotes)'
    [[ "$SIGNING_IDENTITY" == 'Developer ID Application: '* ]] || fail 'Choose a Developer ID Application identity.'
    printf '%s\n' "$identities" | python3 -c '
import re, sys
names = re.findall(r"\"([^\"]+)\"", sys.stdin.read())
if sys.argv[1] not in names:
    sys.exit("The selected identity is not valid in your Keychain. Import its certificate and private key first.")
' "$SIGNING_IDENTITY"
    save_state
    printf 'Certificate configuration saved. Run ./signing/macos/step2.sh.\n'
}

# Parse exit with the call so later file edits cannot change the running flow.
main "$@"; exit "$?"
