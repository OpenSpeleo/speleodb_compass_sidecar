#!/usr/bin/env bash
# Configure a passwordless GitHub identity and save environment secrets via stdin.
# shellcheck source-path=SCRIPTDIR
# shellcheck source=common.sh
source "$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)/common.sh"
require_account
require gh
[[ -n ${SIGNING_PROFILE_ID:-} ]] || fail 'Run step2.sh first.'
gh auth status >/dev/null 2>&1 || fail 'Authenticate with gh auth login first.'
# Use the current repository unless the operator selects another destination.
SIGNING_REPO=${SIGNING_REPO:-$(gh repo view --json nameWithOwner --jq .nameWithOwner)}
[[ "$SIGNING_REPO" =~ ^[A-Za-z0-9_.-]+/[A-Za-z0-9_.-]+$ ]] || fail 'Expected SIGNING_REPO in owner/repository form.'

if [[ -z ${SIGNING_CLIENT_ID:-} ]]; then
    SIGNING_CLIENT_ID=$(az ad app create --display-name 'USAHI Windows Signing' --query appId --output tsv)
    [[ -n "$SIGNING_CLIENT_ID" ]] || fail 'Azure did not return an application ID.'
    save_state
fi
SIGNING_SP_ID=$(az ad sp list --filter "appId eq '$SIGNING_CLIENT_ID'" --query '[0].id' --output tsv)
if [[ -z "$SIGNING_SP_ID" ]]; then
    SIGNING_SP_ID=$(az ad sp create --id "$SIGNING_CLIENT_ID" --query id --output tsv)
fi
[[ -n "$SIGNING_SP_ID" ]] || fail 'Azure did not return a service principal ID.'
save_state

federation_file=$(mktemp "$STATE_DIR/federation.XXXXXX")
trap 'rm -f "$federation_file"' EXIT
# A distinct credential per repository preserves access when another app is added.
credential_name=$(python3 - "$federation_file" "$SIGNING_REPO" <<'PYTHON'
import hashlib, json, sys
from pathlib import Path
subject = f"repo:{sys.argv[2]}:environment:windows-signing"
name = "usahi-github-" + hashlib.sha256(subject.encode()).hexdigest()[:24]
Path(sys.argv[1]).write_text(json.dumps({
    "name": name,
    "issuer": "https://token.actions.githubusercontent.com",
    "subject": subject,
    "audiences": ["api://AzureADTokenExchange"],
}) + "\n")
print(name)
PYTHON
)
credential_id=$(az ad app federated-credential list --id "$SIGNING_CLIENT_ID" \
    --query "[?name=='$credential_name'].id | [0]" --output tsv)
if [[ -n "$credential_id" ]]; then
    az ad app federated-credential update --id "$SIGNING_CLIENT_ID" \
        --federated-credential-id "$credential_id" --parameters "$federation_file" --output none
else
    az ad app federated-credential create --id "$SIGNING_CLIENT_ID" \
        --parameters "$federation_file" --output none
fi
ensure_role "$SIGNING_SP_ID" ServicePrincipal 'Artifact Signing Certificate Profile Signer' "$SIGNING_PROFILE_ID"

# Listing failures must not be mistaken for an absent environment. Leave an
# existing environment and all its deployment protection rules untouched.
environments=$(gh api --paginate "repos/$SIGNING_REPO/environments" --jq '.environments[].name')
if ! printf '%s\n' "$environments" | python3 -c '
import sys
sys.exit(0 if any(line.strip().casefold() == "windows-signing" for line in sys.stdin) else 1)
'; then
    gh api --method PUT "repos/$SIGNING_REPO/environments/windows-signing" >/dev/null
fi

save_secret() {
    printf '%s' "$2" | gh secret set "$1" --repo "$SIGNING_REPO" \
        --env windows-signing >/dev/null
}
save_secret AZURE_CLIENT_ID "$SIGNING_CLIENT_ID"
save_secret AZURE_TENANT_ID "$SIGNING_TENANT_ID"
save_secret AZURE_SUBSCRIPTION_ID "$SIGNING_SUBSCRIPTION_ID"
save_secret AZURE_SIGNING_ENDPOINT "$SIGNING_ENDPOINT"
save_secret AZURE_SIGNING_ACCOUNT "$SIGNING_ACCOUNT"
save_secret AZURE_SIGNING_PROFILE "$SIGNING_PROFILE"
printf '%s\n' 'Azure OIDC identity and windows-signing secrets configured.' \
    'Existing environment protection rules were preserved. A newly created environment has no protection rules.' \
    'The release workflow uses these settings for Azure OIDC and Tauri signing.'
