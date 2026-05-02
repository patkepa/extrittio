# Extrittio CLI

Command line tooling for the Extrittio backend.

## Usage

```bash
cargo run -p extrittio-cli -- auth login --username admin --password admin
cargo run -p extrittio-cli -- devices list
cargo run -p extrittio-cli -- device-types list
cargo run -p extrittio-cli -- fleets list
```

Provision a device and optionally download its mTLS certificate bundle:

```bash
cargo run -p extrittio-cli -- provision \
  --name sensor-001 \
  --device-type sensor \
  --firmware linux-0.1.0 \
  --zenoh-connect tcp/127.0.0.1:7447 \
  --cert-dir ./provisioned/sensor-001
```

The certificate bundle writes:

- `ca.pem`
- `device.pem`
- `device-key.pem`

The backend returns the private key only once. Use
`certs regenerate <device-id> --out-dir <dir>` if the original key was lost.

Provision an ESP32 network analyzer that already has the
`client/c/esp32-network-analyzer-idf-c` firmware flashed:

```bash
cargo run -p extrittio-cli -- provision \
  --name analyzer-001 \
  --device-type esp32-network-analyzer \
  --firmware v1.0.0-network-analyzer-c \
  --zenoh-connect tcp/192.168.0.10:7447 \
  --wifi-ssid "$WIFI_SSID" \
  --wifi-password "$WIFI_PASSWORD" \
  --flash-esp32-nvs
```

`--flash-esp32-nvs` writes the default ESP-IDF NVS partition at `0x9000`
with the `extrittio` namespace keys consumed by the firmware:
`device_id`, `wifi_ssid`, `wifi_pass`, `zenoh`, and `fw_version`.
`--device-type` resolves an existing type by name or creates it. Pass `--port`
if more than one USB serial device is connected. Run the command from an
ESP-IDF shell, or pass `--idf-path` and `--idf-python` explicitly.

## Configuration

The CLI saves local auth state in `~/.config/extrittio/cli.json` unless
`EXTRITTIO_CLI_CONFIG` or `--config` is set.

Connection values can be supplied through flags or environment variables:

```bash
EXTRITTIO_URL=http://localhost:8080
EXTRITTIO_TOKEN=<jwt>
```

Use `--output json` for scripts.
