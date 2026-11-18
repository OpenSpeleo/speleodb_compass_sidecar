#!/usr/bin/env bash
# Shared by the executable setup steps; configuration stays outside the checkout.
set +x
set -euo pipefail
umask 077

fail() { printf 'Error: %s\n' "$*" >&2; exit 1; }
require() { command -v "$1" >/dev/null 2>&1 || fail "Install $1 before running this step."; }
require az
require python3

STATE_DIR="${XDG_CONFIG_HOME:-$HOME/.config}/usahi/windows-signing"
STATE_FILE="$STATE_DIR/state.sh"
python3 - "$STATE_DIR" <<'PY'
from pathlib import Path
import sys
path = Path(sys.argv[1]).expanduser().resolve()
if any((parent / '.git').exists() for parent in (path, *path.parents)):
    sys.exit('Signing configuration must be outside every Git checkout. Change XDG_CONFIG_HOME.')
PY

if [[ -e "$STATE_FILE" ]]; then
    # This private file is written by save_state using Bash-escaped assignments.
    # shellcheck source=/dev/null
    source "$STATE_FILE"
fi

SIGNING_RESOURCE_GROUP=${SIGNING_RESOURCE_GROUP:-usahi-signing}
SIGNING_LOCATION=${SIGNING_LOCATION:-eastus}
SIGNING_PROFILE=${SIGNING_PROFILE:-usahi-public}

save_state() {
    mkdir -p "$STATE_DIR"
    chmod 700 "$STATE_DIR"
    local temporary name
    temporary=$(mktemp "$STATE_DIR/state.XXXXXX")
    for name in SIGNING_SUBSCRIPTION_ID SIGNING_TENANT_ID SIGNING_ACCOUNT SIGNING_RESOURCE_GROUP \
        SIGNING_LOCATION SIGNING_PROFILE SIGNING_ACCOUNT_ID SIGNING_ENDPOINT SIGNING_VALIDATION_ID \
        SIGNING_PROFILE_ID SIGNING_CLIENT_ID SIGNING_SP_ID; do
        printf '%s=%q\n' "$name" "${!name-}" >> "$temporary"
    done
    chmod 600 "$temporary"
    mv "$temporary" "$STATE_FILE"
}

prompt_identifier() {
    local variable=$1 label=$2 value
    read -r -p "$label: " value || fail 'Input canceled.'
    [[ -n "$value" ]] || fail 'A nonempty value is required.'
    printf -v "$variable" '%s' "$value"
}

select_subscription() {
    [[ -n ${SIGNING_SUBSCRIPTION_ID:-} ]] || fail 'Run step1.sh first.'
    az account set --subscription "$SIGNING_SUBSCRIPTION_ID"
    SIGNING_TENANT_ID=$(az account show --query tenantId --output tsv)
    [[ -n "$SIGNING_TENANT_ID" ]] || fail 'Azure did not return a tenant ID.'
}

require_account() {
    [[ -n ${SIGNING_ACCOUNT_ID:-} && -n ${SIGNING_ACCOUNT:-} ]] || fail 'Run step1.sh first.'
    select_subscription
    require_signing_extension
}

require_signing_extension() {
    az extension show --name artifact-signing --output none 2>/dev/null ||
        fail 'Install the CLI extension: az extension add --name artifact-signing'
}

ensure_role() {
    local principal=$1 kind=$2 role=$3 scope=$4 assignment
    assignment=$(az role assignment list --scope "$scope" \
        --query "[?principalId=='$principal' && roleDefinitionName=='$role'].id | [0]" --output tsv)
    if [[ -z "$assignment" ]]; then
        az role assignment create --assignee-object-id "$principal" --assignee-principal-type "$kind" \
            --role "$role" --scope "$scope" --output none
    fi
}
