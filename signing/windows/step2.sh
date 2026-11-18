#!/usr/bin/env bash
# Run after Public identity validation is approved in Azure Portal.
# shellcheck source-path=SCRIPTDIR
# shellcheck source=common.sh
source "$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)/common.sh"

# Load the full flow before starting Azure operations.
main() {
    require_account

    if [[ -z ${SIGNING_VALIDATION_ID:-} ]]; then
        prompt_identifier SIGNING_VALIDATION_ID 'Approved identity validation ID from Azure Portal'
    fi
    save_state

    profiles=$(az artifact-signing certificate-profile list --resource-group "$SIGNING_RESOURCE_GROUP" \
        --account-name "$SIGNING_ACCOUNT" --output json)
    profile_type=$(printf '%s' "$profiles" | python3 -c '
import json, sys
name = sys.argv[1].casefold()
for profile in json.load(sys.stdin):
    if profile["name"].casefold() == name:
        print(profile["profileType"])
        break
    ' "$SIGNING_PROFILE")
    if [[ -n "$profile_type" ]]; then
        [[ "$profile_type" == PublicTrust ]] || fail 'Existing profile is not PublicTrust; it was left unchanged.'
    else
        az artifact-signing certificate-profile create --resource-group "$SIGNING_RESOURCE_GROUP" \
            --account-name "$SIGNING_ACCOUNT" --name "$SIGNING_PROFILE" --profile-type PublicTrust \
            --identity-validation-id "$SIGNING_VALIDATION_ID" --include-street-address false \
            --include-postal-code false --output none
    fi
    SIGNING_PROFILE_ID=$(az artifact-signing certificate-profile show --resource-group "$SIGNING_RESOURCE_GROUP" \
        --account-name "$SIGNING_ACCOUNT" --name "$SIGNING_PROFILE" --query id --output tsv)
    [[ -n "$SIGNING_PROFILE_ID" ]] || fail 'Azure did not return a certificate profile ID.'
    save_state
    printf 'Public Trust profile ready. Run ./signing/windows/step3.sh.\n'
}

# Parse exit with the call so Bash does not reread this file after provisioning.
main "$@"; exit "$?"
