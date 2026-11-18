#!/usr/bin/env bash
# Shared operator setup; configuration and credentials stay outside checkouts.
set +x
set -euo pipefail
umask 077

fail() { printf 'Error: %s\n' "$*" >&2; exit 1; }
require() { command -v "$1" >/dev/null 2>&1 || fail "Install $1 before running this step."; }
[[ $(uname -s) == Darwin ]] || fail 'Run these steps on macOS.'
require python3

outside_checkout() {
    python3 - "$1" <<'PY'
from pathlib import Path
import sys
path = Path(sys.argv[1]).expanduser().resolve()
if any((parent / '.git').exists() for parent in (path, *path.parents)):
    sys.exit('Keep signing configuration and private files outside Git checkouts.')
print(path)
PY
}

STATE_DIR=$(outside_checkout "${XDG_CONFIG_HOME:-$HOME/.config}/usahi/macos-signing")
STATE_FILE="$STATE_DIR/state.sh"
if [[ -e "$STATE_FILE" ]]; then
    # Written by save_state with Bash-escaped assignments.
    # shellcheck source=/dev/null
    source "$STATE_FILE"
fi
SIGNING_KEYCHAIN_PROFILE=${SIGNING_KEYCHAIN_PROFILE:-usahi-notarization}

save_state() {
    mkdir -p "$STATE_DIR"
    chmod 700 "$STATE_DIR"
    local temporary name
    temporary=$(mktemp "$STATE_DIR/state.XXXXXX")
    for name in SIGNING_CERTIFICATE_PATH SIGNING_IDENTITY SIGNING_API_KEY_PATH \
        SIGNING_API_KEY_ID SIGNING_API_ISSUER SIGNING_KEYCHAIN_PROFILE; do
        printf '%s=%q\n' "$name" "${!name-}" >> "$temporary"
    done
    chmod 600 "$temporary"
    mv "$temporary" "$STATE_FILE"
}

prompt_value() {
    local variable=$1 label=$2 value current
    current=${!variable-}
    if [[ -n "$current" ]]; then
        label="$label [$current]"
    fi
    read -r -p "$label: " value || fail 'Input canceled.'
    value=${value:-$current}
    [[ -n "$value" ]] || fail 'A nonempty value is required.'
    printf -v "$variable" '%s' "$value"
}

private_file() {
    local path
    path=$(outside_checkout "$1")
    [[ -f "$path" && -r "$path" ]] || fail 'Expected a readable credential file.'
    printf '%s\n' "$path"
}

require_setup() {
    [[ -n ${SIGNING_CERTIFICATE_PATH:-} && -n ${SIGNING_IDENTITY:-} ]] || fail 'Run step1.sh first.'
    [[ -n ${SIGNING_API_KEY_PATH:-} && -n ${SIGNING_API_KEY_ID:-} && -n ${SIGNING_API_ISSUER:-} ]] ||
        fail 'Run step2.sh first.'
    SIGNING_CERTIFICATE_PATH=$(private_file "$SIGNING_CERTIFICATE_PATH")
    SIGNING_API_KEY_PATH=$(private_file "$SIGNING_API_KEY_PATH")
}
