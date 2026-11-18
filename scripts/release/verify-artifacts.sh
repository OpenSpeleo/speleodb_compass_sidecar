#!/usr/bin/env bash
set -euo pipefail

usage() {
  cat <<'USAGE'
Usage: verify-artifacts.sh MODE --platform PLATFORM --release-root DIRECTORY

Modes:
  updater-signatures  Verify updater payloads and signature files.
  runtime             Verify the signed and notarized macOS package.
  all                 Run updater-signatures and runtime.
USAGE
}

platform=""
release_root=""
mode="${1:-}"
if [ -z "$mode" ]; then
  usage >&2
  exit 2
fi
if [ "$mode" = "--help" ] || [ "$mode" = "-h" ]; then
  usage
  exit 0
fi
shift

while [ "$#" -gt 0 ]; do
  case "$1" in
    --platform)
      platform="${2:?missing value for --platform}"
      shift 2
      ;;
    --release-root)
      release_root="${2:?missing value for --release-root}"
      shift 2
      ;;
    --help|-h)
      usage
      exit 0
      ;;
    *)
      echo "error: unknown option: $1" >&2
      usage >&2
      exit 2
      ;;
  esac
done

if [ -z "$platform" ] || [ -z "$release_root" ]; then
  echo "error: --platform and --release-root are required" >&2
  usage >&2
  exit 2
fi
case "$platform" in
  macos-latest|windows-latest) ;;
  *) echo "error: unsupported release platform: $platform" >&2; exit 2 ;;
esac
shopt -s nullglob

reject_match() {
  local description="$1"
  local pattern="$2"
  if compgen -G "$pattern" >/dev/null; then
    echo "error: ${description} must not be produced with pattern ${pattern}" >&2
    exit 1
  fi
}

verify_signature_shape() {
  local signature_path="$1"
  bun - "$signature_path" <<'JS'
const fs = require("node:fs");

const signaturePath = process.argv[2];
const value = fs.readFileSync(signaturePath, "utf8").trim();

if (!value) {
  console.error(`error: ${signaturePath} is empty`);
  process.exit(1);
}

if (value.includes("\n") || value.includes("\r")) {
  console.error(`error: ${signaturePath} must be a single-line signature token`);
  process.exit(1);
}
JS
}

verify_payload() {
  local description="$1"
  shift
  if [ "$#" -ne 1 ] || [ ! -f "$1" ]; then
    echo "error: expected exactly one $description, found $#" >&2
    exit 1
  fi
  local signature_path="$1.sig"
  if [ ! -f "$signature_path" ]; then
    echo "error: missing updater signature: $signature_path" >&2
    exit 1
  fi
  verify_signature_shape "$signature_path"
}

verify_updater_signatures() {
  case "$platform" in
    macos-latest)
      verify_payload "macOS updater archive" "$release_root"/bundle/macos/*.app.tar.gz
      ;;
    windows-latest)
      verify_payload "Windows NSIS updater artifact" "$release_root"/bundle/nsis/*.exe
      verify_payload "Windows MSI updater artifact" "$release_root"/bundle/msi/*.msi
      reject_match "Windows v1-compatible NSIS archive" "$release_root/bundle/nsis/*.nsis.zip"
      reject_match "Windows v1-compatible NSIS archive signature" "$release_root/bundle/nsis/*.nsis.zip.sig"
      reject_match "Windows v1-compatible MSI archive" "$release_root/bundle/msi/*.msi.zip"
      reject_match "Windows v1-compatible MSI archive signature" "$release_root/bundle/msi/*.msi.zip.sig"
      ;;
  esac
}

verify_macos_runtime() (
  local dmg_path
  local mount_dir
  local app_path
  local dmgs=("$release_root"/bundle/dmg/*.dmg)
  if [ "${#dmgs[@]}" -ne 1 ] || [ ! -f "${dmgs[0]}" ]; then
    echo "error: expected exactly one macOS DMG, found ${#dmgs[@]}" >&2
    exit 1
  fi
  dmg_path="${dmgs[0]}"
  mount_dir="$(mktemp -d "${RUNNER_TEMP:-${TMPDIR:-/tmp}}/sidecar-verify-dmg.XXXXXX")"
  # mount reports physical paths (/private/var on macOS), without trailing //.
  mount_dir="$(cd "$mount_dir" && pwd -P)"
  local attached=false

  # ShellCheck does not follow EXIT traps inside a subshell function.
  # shellcheck disable=SC2329
  cleanup() {
    local status=$?
    if [ "$attached" = true ]; then
      hdiutil detach "$mount_dir" -quiet || hdiutil detach "$mount_dir" -force -quiet || true
    fi
    rm -rf "$mount_dir"
    exit "$status"
  }
  trap 'cleanup' EXIT

  hdiutil attach "$dmg_path" -mountpoint "$mount_dir" -nobrowse -quiet
  attached=true
  local apps=("$mount_dir"/*.app)
  if [ "${#apps[@]}" -ne 1 ] || [ ! -d "${apps[0]}" ]; then
    echo "error: expected exactly one application in the DMG" >&2
    exit 1
  fi
  app_path="${apps[0]}"
  test -f "$app_path/Contents/MacOS/speleodb-compass-sidecar"
  codesign --verify --deep --strict --verbose=2 "$app_path"
  # Assess the shipped app and its container, not just the build-tree app.
  spctl --assess --type execute --verbose=2 "$app_path"
  xcrun stapler validate "$app_path"
  spctl --assess --type open --context context:primary-signature --verbose=2 "$dmg_path"
  xcrun stapler validate "$dmg_path"
)

verify_runtime() {
  case "$platform" in
    macos-latest) verify_macos_runtime ;;
    windows-latest)
      # Tauri's signing callback verifies every Windows signature before upload.
      ;;
  esac
}

case "$mode" in
  updater-signatures) verify_updater_signatures ;;
  runtime) verify_runtime ;;
  all) verify_updater_signatures; verify_runtime ;;
  *) echo "error: unsupported verification mode: $mode" >&2; exit 2 ;;
esac
