#!/usr/bin/env bash
# Run directly from zsh or Bash: ./signing/windows/step1.sh
set +x
# Set your signing account name here; this identifier is not a secret.
SIGNING_ACCOUNT="usahi-signing"
requested_account=$SIGNING_ACCOUNT
# shellcheck source-path=SCRIPTDIR
# shellcheck source=common.sh
source "$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)/common.sh"

[[ -n "$requested_account" && "$requested_account" != placeholder ]] ||
    fail 'Set SIGNING_ACCOUNT at the top of step1.sh before running it.'

if [[ "$requested_account" != "${SIGNING_ACCOUNT:-}" ]]; then
    SIGNING_ACCOUNT=$requested_account
    # These identifiers belong to the previous account; retain the CI identity.
    unset SIGNING_ACCOUNT_ID SIGNING_ENDPOINT SIGNING_VALIDATION_ID SIGNING_PROFILE_ID
fi

printf 'Using your current Azure login. This step creates a paid Basic signing account if absent.\n'
if [[ -z ${SIGNING_SUBSCRIPTION_ID:-} ]]; then
    SIGNING_SUBSCRIPTION_ID=$(az account show --query id --output tsv) ||
        fail 'Log in and select your subscription with az login, then rerun this script.'
fi
select_subscription
# Persist immediately so reruns retain the selected account after a later failure.
save_state

if ! az extension show --name artifact-signing --output none 2>/dev/null; then
    az extension add --name artifact-signing --output none
fi
az provider register --namespace Microsoft.CodeSigning --wait --output none
az group create --name "$SIGNING_RESOURCE_GROUP" --location "$SIGNING_LOCATION" --output none

# A failed listing is an error, never permission to create a replacement account.
accounts=$(az artifact-signing list --resource-group "$SIGNING_RESOURCE_GROUP" --output json)
exists=$(printf '%s' "$accounts" | python3 -c '
import json, sys
name = sys.argv[1].casefold()
print("yes" if any(a["name"].casefold() == name for a in json.load(sys.stdin)) else "no")
' "$SIGNING_ACCOUNT")
if [[ "$exists" == no ]]; then
    az artifact-signing create --resource-group "$SIGNING_RESOURCE_GROUP" \
        --name "$SIGNING_ACCOUNT" --location "$SIGNING_LOCATION" --sku Basic --output none
fi
SIGNING_ACCOUNT_ID=$(az artifact-signing show --resource-group "$SIGNING_RESOURCE_GROUP" \
    --name "$SIGNING_ACCOUNT" --query id --output tsv)
SIGNING_ENDPOINT=$(az artifact-signing show --resource-group "$SIGNING_RESOURCE_GROUP" \
    --name "$SIGNING_ACCOUNT" --query accountUri --output tsv)
[[ -n "$SIGNING_ACCOUNT_ID" && -n "$SIGNING_ENDPOINT" ]] || fail 'Azure returned incomplete account details.'
save_state

user_id=$(az ad signed-in-user show --query id --output tsv)
ensure_role "$user_id" User 'Artifact Signing Identity Verifier' "$SIGNING_ACCOUNT_ID"

printf '%s\n' 'Account ready. In Azure Portal, open this signing account, then Identity validations.' \
    'Create a Public validation and wait for approval. Then run ./signing/windows/step2.sh.'
