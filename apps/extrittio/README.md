# Extrittio

Command line tooling for operating and administering Extrittio.

## Usage

Install the operator binary from the repository root:

```bash
cargo install --path apps/extrittio
```

Or from the `apps/extrittio/` directory:

```bash
cargo install --path .
```

```bash
cargo run -p extrittio
cargo run -p extrittio -- serve
cargo run -p extrittio -- migrate
cargo run -p extrittio -- init
cargo run -p extrittio -- auth login --username admin --password admin
cargo run -p extrittio -- devices list
cargo run -p extrittio -- device-types list
cargo run -p extrittio -- fleets list
```

For an installed release binary, the same commands are:

```bash
extrittio
extrittio serve
extrittio migrate
extrittio init
extrittio auth login --username admin --password admin
```

Running `extrittio` with no subcommand defaults to `extrittio serve`. `serve`
starts the backend service, runs startup initialization, opens Zenoh, starts
background workers, serves the REST API, and serves the React UI from
`apps/frontend/dist` when that build directory exists. Override the UI directory with
`EXTRITTIO_UI_DIR` or `extrittio serve --ui-dir <path>`, or pass `--no-ui` for
API-only mode. `migrate` only applies pending database migrations. `init`
applies migrations, seeds built-in records, creates the initial admin user when
needed, and writes service certificates.

For the installed, single-node appliance experience, use the repository Make
target. It builds the embedded UI, the hobby binary, and a pinned OpenThread
Border Router agent in the location used automatically by `extrittio run`:

```bash
make hobby
extrittio run
```

`make install-extrittio` is an equivalent target. A direct Cargo hobby install
is suitable for development, but does not package `otbr-agent`; pass
`--thread-otbr-agent <path>` in that case.

`run` fixes the backend/profile to local Turso, embeds the web UI, creates the
first-run owner as `admin` / `admin`, and starts the complete stack. Change the
default password after signing in.

The hobby binary includes OpenThread Border Router supervision. `extrittio run`
detects one connected RCP and starts its `otbr-agent` runtime, then listens on
IPv6. It remains Wi-Fi-only when no RCP is connected. Make Thread mandatory
with `--thread-required`:

```bash
extrittio run --thread-required
```

Pass `--thread-rcp /dev/cu.usbmodem…` when auto-discovery is ambiguous, and
`--thread-infra-interface en0` when macOS does not use its usual primary
interface. Provision a Thread device with an IPv6 Zenoh locator such as
`tls/[fdxx:...]:7447`. The backend receives the same Extrittio Zenoh topics and
payloads as it does from Wi-Fi devices. See [OpenThread hobby deployment](../../docs/OPENTHREAD_HOBBY.md)
for RCP firmware, runtime packaging, macOS/Linux configuration, and verification.

Publish a firmware binary and trigger OTA:

```bash
# Backend should expose an address devices can fetch, not localhost from the device's view.
EXTRITTIO_PUBLIC_URL=http://192.0.2.20:8080 cargo run -p extrittio -- serve

cargo run -p extrittio -- firmware upload \
  --device-type-id 1 \
  --version esp32-network-analyzer-c-0.2.0 \
  --file clients/c/esp32-network-analyzer-idf-c/build/extrittio-esp32-network-analyzer-c.bin

cargo run -p extrittio -- ota deploy \
  --device esp32-network-analyzer-001 \
  --firmware-id 42

cargo run -p extrittio -- ota wait \
  --device esp32-network-analyzer-001 \
  --firmware-id 42
```

Provision a device and optionally download its mTLS certificate bundle:

```bash
cargo run -p extrittio -- provision \
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
`clients/c/esp32-network-analyzer-idf-c` firmware flashed:

```bash
cargo run -p extrittio -- provision \
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

## Internal Structure

The operator binary is split into layers so new command families can be added without
expanding the binary entrypoint:

- `args.rs` owns the public command shape and clap flags.
- `commands/` owns domain command execution.
- `api.rs` owns authenticated HTTP transport and API error handling.
- `models.rs` owns backend response DTOs.
- `output.rs` owns table/JSON rendering.
- `config.rs` owns local CLI state.
- `esp32.rs` owns provisioning support for ESP-IDF targets.

For the single-executable Turso hobby build, data layout and backup/restore commands, see [Turso Hobby Deployment](../../docs/TURSO_HOBBY.md).
