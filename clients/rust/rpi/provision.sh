#!/usr/bin/env bash
# Provision from a published blueprint and install the contract-native Pi client.
set -euo pipefail
umask 077

usage() {
    printf '%s\n' 'Usage: provision.sh --backend-url URL --token-file PATH --blueprint-revision-id ID --device-name NAME --binary PATH [--fleet-id ID] [--install-dir DIR]'
}
fail() { printf 'Error: %s\n' "$*" >&2; exit 1; }
BACKEND_URL=""
TOKEN_FILE=""
REVISION_ID=""
DEVICE_NAME=""
BINARY_PATH=""
FLEET_ID=""
INSTALL_DIR="/opt/extrittio"
SERVICE_PATH="/etc/systemd/system/extrittio-rpi.service"

while [[ $# -gt 0 ]]; do
    case "$1" in
        --help|-h) usage; exit 0 ;;
        --backend-url|--token-file|--blueprint-revision-id|--device-name|--binary|--fleet-id|--install-dir)
            [[ $# -ge 2 && -n "$2" ]] || fail "Missing value for $1"
            case "$1" in
                --backend-url) BACKEND_URL="$2" ;;
                --token-file) TOKEN_FILE="$2" ;;
                --blueprint-revision-id) REVISION_ID="$2" ;;
                --device-name) DEVICE_NAME="$2" ;;
                --binary) BINARY_PATH="$2" ;;
                --fleet-id) FLEET_ID="$2" ;;
                --install-dir) INSTALL_DIR="$2" ;;
            esac
            shift 2 ;;
        *) fail "Unknown option: $1" ;;
    esac
done
[[ -n "$BACKEND_URL" && -n "$TOKEN_FILE" && -n "$REVISION_ID" && -n "$DEVICE_NAME" && -n "$BINARY_PATH" ]] || { usage >&2; fail 'Required option missing'; }
[[ "$BACKEND_URL" =~ ^https?://[^[:space:]]+$ ]] || fail 'Backend URL must use HTTP or HTTPS'
[[ "$INSTALL_DIR" =~ ^/[a-zA-Z0-9/_-]+$ && "$INSTALL_DIR" != / ]] || fail 'Install directory must be an absolute path without spaces or systemd specifiers'
[[ -z "$FLEET_ID" || "$FLEET_ID" =~ ^[1-9][0-9]*$ ]] || fail 'Fleet ID must be a positive integer'
[[ -f "$BINARY_PATH" && -x "$BINARY_PATH" ]] || fail 'Binary must be an executable regular file'
[[ -f "$TOKEN_FILE" && -r "$TOKEN_FILE" ]] || fail 'Token file must be readable'
for dependency in curl jq sudo systemctl install mktemp; do
    command -v "$dependency" >/dev/null || fail "Missing dependency: $dependency"
done
# Never overwrite another installation or rotate an existing device identity.
[[ ! -e "$SERVICE_PATH" && ! -e "$INSTALL_DIR/extrittio-rpi" && ! -e "$INSTALL_DIR/device-contract.json" ]] || fail 'An installation already exists; inspect it before reprovisioning'
BINARY_PATH=$(cd "$(dirname "$BINARY_PATH")" && printf '%s/%s' "$PWD" "$(basename "$BINARY_PATH")")
TOKEN=$(<"$TOKEN_FILE")
[[ "$TOKEN" =~ ^[a-zA-Z0-9._~-]+$ ]] || fail 'Expected a single bearer token without whitespace'
TEMP_DIR=$(mktemp -d)
trap 'rm -rf -- "$TEMP_DIR"' EXIT
printf 'Authorization: Bearer %s\n' "$TOKEN" > "$TEMP_DIR/auth-header"
unset TOKEN
BACKEND_URL=${BACKEND_URL%/}
sudo -v

jq -n --arg name "$DEVICE_NAME" --arg revision "$REVISION_ID" --arg fleet "$FLEET_ID" \
    '{name: $name, blueprint_revision_id: $revision} + (if $fleet == "" then {} else {fleet_id: ($fleet | tonumber)} end)' > "$TEMP_DIR/request.json"
# No automatic POST retry: an ambiguous response requires inspecting the server.
curl --fail --silent --show-error --connect-timeout 15 --max-time 60 \
    --header "@$TEMP_DIR/auth-header" --header 'Content-Type: application/json' \
    --data-binary "@$TEMP_DIR/request.json" "$BACKEND_URL/api/v1/devices" > "$TEMP_DIR/device.json"
DEVICE_ID=$(jq -er '.id | select(type == "string" and test("^[a-zA-Z0-9_-]+$"))' "$TEMP_DIR/device.json")
printf 'Created device %s. If a later step fails, retain this ID; do not blindly rerun provisioning.\n' "$DEVICE_ID"
curl --fail --silent --show-error --connect-timeout 15 --max-time 60 \
    --header "@$TEMP_DIR/auth-header" "$BACKEND_URL/api/v1/devices/$DEVICE_ID/contract" > "$TEMP_DIR/device-contract.json"
jq -e --arg id "$DEVICE_ID" --arg revision "$REVISION_ID" \
    '.device_id == $id and .blueprint_revision_id == $revision and .document.deviceId == $id' "$TEMP_DIR/device-contract.json" >/dev/null
"$BINARY_PATH" --contract "$TEMP_DIR/device-contract.json" --device-id "$DEVICE_ID" --check-contract

sudo install -d -m 755 "$INSTALL_DIR"
sudo install -m 755 "$BINARY_PATH" "$INSTALL_DIR/extrittio-rpi"
sudo install -m 600 "$TEMP_DIR/device-contract.json" "$INSTALL_DIR/device-contract.json"
printf '%s\n' \
    '[Unit]' 'Description=Extrittio contract-native Raspberry Pi client' \
    'After=network-online.target' 'Wants=network-online.target' '' \
    '[Service]' 'Type=simple' \
    "ExecStart=$INSTALL_DIR/extrittio-rpi --contract $INSTALL_DIR/device-contract.json" \
    'Restart=on-failure' 'RestartSec=5' \
    '# The bounded OTA rollback watchdog must survive a client restart.' \
    'KillMode=process' 'Environment=RUST_LOG=extrittio_rpi=info' '' \
    '[Install]' 'WantedBy=multi-user.target' > "$TEMP_DIR/extrittio-rpi.service"
sudo install -m 644 "$TEMP_DIR/extrittio-rpi.service" "$SERVICE_PATH"
sudo systemctl daemon-reload
sudo systemctl enable --now extrittio-rpi.service
printf 'Provisioned %s using revision %s. Contract installed at %s/device-contract.json\n' "$DEVICE_ID" "$REVISION_ID" "$INSTALL_DIR"
