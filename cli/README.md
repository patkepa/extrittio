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
  --device-type-id 1 \
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

## Configuration

The CLI saves local auth state in `~/.config/extrittio/cli.json` unless
`EXTRITTIO_CLI_CONFIG` or `--config` is set.

Connection values can be supplied through flags or environment variables:

```bash
EXTRITTIO_URL=http://localhost:8080
EXTRITTIO_TOKEN=<jwt>
```

Use `--output json` for scripts.
