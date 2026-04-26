#!/usr/bin/env bash
#
# Extrittio RPi Provisioning Script
#
# Registers the device with the backend, installs the binary, and sets up
# a systemd service. Run once on each Raspberry Pi.
#
# Usage:
#   ./provision.sh --backend-url http://192.0.2.10:3000 --binary ./extrittio-rpi
#
# Options:
#   --backend-url URL     Backend API base URL (required)
#   --binary PATH         Path to the extrittio-rpi binary (required)
#   --device-name NAME    Human-readable name (default: rpi-<serial>)
#   --fleet-id ID         Fleet ID to assign the device to
#   --connect ENDPOINT    Zenoh endpoint (default: tcp/<backend-host>:7447)
#   --install-dir DIR     Installation directory (default: /opt/extrittio)
#
set -euo pipefail

# --- Defaults ---
INSTALL_DIR="/opt/extrittio"
DEVICE_NAME=""
FLEET_ID=""
BACKEND_URL=""
BINARY_PATH=""
ZENOH_CONNECT=""
DEVICE_TYPE_NAME="raspberry-pi"
SERVICE_NAME="extrittio-rpi"

# --- Parse args ---
while [[ $# -gt 0 ]]; do
    case "$1" in
        --backend-url)  BACKEND_URL="$2";  shift 2 ;;
        --binary)       BINARY_PATH="$2";  shift 2 ;;
        --device-name)  DEVICE_NAME="$2";  shift 2 ;;
        --fleet-id)     FLEET_ID="$2";     shift 2 ;;
        --connect)      ZENOH_CONNECT="$2"; shift 2 ;;
        --install-dir)  INSTALL_DIR="$2";  shift 2 ;;
        *)
            echo "Unknown option: $1" >&2
            exit 1
            ;;
    esac
done

if [[ -z "$BACKEND_URL" ]]; then
    echo "Error: --backend-url is required" >&2
    exit 1
fi

if [[ -z "$BINARY_PATH" ]]; then
    echo "Error: --binary is required" >&2
    exit 1
fi

if [[ ! -f "$BINARY_PATH" ]]; then
    echo "Error: binary not found at $BINARY_PATH" >&2
    exit 1
fi

# --- Detect RPi serial number ---
get_serial() {
    # Try device-tree first (RPi 4/5)
    if [[ -f /sys/firmware/devicetree/base/serial-number ]]; then
        tr -d '\0' < /sys/firmware/devicetree/base/serial-number
        return
    fi
    # Fallback to /proc/cpuinfo (RPi 3 and older)
    if grep -q "^Serial" /proc/cpuinfo 2>/dev/null; then
        grep "^Serial" /proc/cpuinfo | awk '{print $3}'
        return
    fi
    # Last resort: use machine-id
    if [[ -f /etc/machine-id ]]; then
        cat /etc/machine-id
        return
    fi
    echo "Error: cannot determine device serial number" >&2
    exit 1
}

SERIAL=$(get_serial)
echo "RPi serial: $SERIAL"

if [[ -z "$DEVICE_NAME" ]]; then
    DEVICE_NAME="rpi-${SERIAL: -8}"
fi

# Derive Zenoh endpoint from backend URL if not specified
if [[ -z "$ZENOH_CONNECT" ]]; then
    BACKEND_HOST=$(echo "$BACKEND_URL" | sed -E 's|https?://||; s|:[0-9]+$||; s|/.*||')
    ZENOH_CONNECT="tcp/${BACKEND_HOST}:7447"
fi

echo "Device name:    $DEVICE_NAME"
echo "Backend URL:    $BACKEND_URL"
echo "Zenoh endpoint: $ZENOH_CONNECT"
echo "Install dir:    $INSTALL_DIR"

# --- Ensure device type exists ---
echo ""
echo "==> Ensuring device type '$DEVICE_TYPE_NAME' exists..."

# List existing device types and check
TYPES_RESPONSE=$(curl -sf "${BACKEND_URL}/api/v1/device-types?limit=100" || echo '{"data":[]}')
DEVICE_TYPE_ID=$(echo "$TYPES_RESPONSE" | python3 -c "
import sys, json
data = json.load(sys.stdin)
items = data.get('data', data) if isinstance(data, dict) else data
for t in items:
    if t.get('name') == '$DEVICE_TYPE_NAME':
        print(t['id'])
        sys.exit(0)
print('')
" 2>/dev/null || echo "")

if [[ -z "$DEVICE_TYPE_ID" ]]; then
    echo "    Creating device type '$DEVICE_TYPE_NAME'..."
    CREATE_TYPE_RESPONSE=$(curl -sf -X POST "${BACKEND_URL}/api/v1/device-types" \
        -H "Content-Type: application/json" \
        -d "{\"name\": \"$DEVICE_TYPE_NAME\"}")

    DEVICE_TYPE_ID=$(echo "$CREATE_TYPE_RESPONSE" | python3 -c "
import sys, json
data = json.load(sys.stdin)
print(data.get('id', ''))
" 2>/dev/null || echo "")

    if [[ -z "$DEVICE_TYPE_ID" ]]; then
        echo "Error: failed to create device type" >&2
        echo "Response: $CREATE_TYPE_RESPONSE" >&2
        exit 1
    fi
    echo "    Created with id=$DEVICE_TYPE_ID"
else
    echo "    Already exists with id=$DEVICE_TYPE_ID"
fi

# --- Register device ---
echo ""
echo "==> Registering device '$DEVICE_NAME'..."

DEVICE_PAYLOAD="{\"name\": \"$DEVICE_NAME\", \"device_type_id\": $DEVICE_TYPE_ID"
if [[ -n "$FLEET_ID" ]]; then
    DEVICE_PAYLOAD="$DEVICE_PAYLOAD, \"fleet_id\": $FLEET_ID"
fi
DEVICE_PAYLOAD="$DEVICE_PAYLOAD}"

DEVICE_RESPONSE=$(curl -sf -X POST "${BACKEND_URL}/api/v1/devices" \
    -H "Content-Type: application/json" \
    -d "$DEVICE_PAYLOAD")

DEVICE_ID=$(echo "$DEVICE_RESPONSE" | python3 -c "
import sys, json
data = json.load(sys.stdin)
print(data.get('id', ''))
" 2>/dev/null || echo "")

if [[ -z "$DEVICE_ID" ]]; then
    echo "Error: failed to register device" >&2
    echo "Response: $DEVICE_RESPONSE" >&2
    exit 1
fi

echo "    Registered with id=$DEVICE_ID"

# --- Install binary ---
echo ""
echo "==> Installing binary to $INSTALL_DIR..."

sudo mkdir -p "$INSTALL_DIR"
sudo cp "$BINARY_PATH" "${INSTALL_DIR}/extrittio-rpi"
sudo chmod 755 "${INSTALL_DIR}/extrittio-rpi"

echo "    Binary installed"

# --- Create systemd service ---
echo ""
echo "==> Creating systemd service '$SERVICE_NAME'..."

sudo tee "/etc/systemd/system/${SERVICE_NAME}.service" > /dev/null <<EOF
[Unit]
Description=Extrittio RPi Telemetry Client
After=network-online.target
Wants=network-online.target

[Service]
Type=simple
ExecStart=${INSTALL_DIR}/extrittio-rpi --device-id ${DEVICE_ID} --connect ${ZENOH_CONNECT}
Restart=always
RestartSec=5
Environment=RUST_LOG=extrittio_rpi=info

[Install]
WantedBy=multi-user.target
EOF

sudo systemctl daemon-reload
sudo systemctl enable "$SERVICE_NAME"
sudo systemctl start "$SERVICE_NAME"

echo "    Service started"

# --- Done ---
echo ""
echo "=== Provisioning complete ==="
echo "  Device ID:   $DEVICE_ID"
echo "  Device Name: $DEVICE_NAME"
echo "  Serial:      $SERIAL"
echo "  Service:     $SERVICE_NAME"
echo ""
echo "Useful commands:"
echo "  sudo systemctl status $SERVICE_NAME"
echo "  sudo journalctl -u $SERVICE_NAME -f"
