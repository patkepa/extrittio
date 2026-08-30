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
target. It builds the embedded UI, the Extrittio Edge binary, and a pinned OpenThread
Border Router agent in the location used automatically by `extrittio run`:

```bash
cargo xtask install edge --release
extrittio run
```

Omit `--release` for a debug build. For a repository development build, run
`cargo xtask otbr build`; `target/debug/extrittio` and
`target/release/extrittio` discover that pinned build automatically.

`run` fixes the backend/profile to local Turso, embeds the web UI, creates the
first-run owner as `admin` / `admin`, and starts the complete stack. Change the
default password after signing in.

The Extrittio Edge binary includes OpenThread Border Router supervision. `extrittio run`
detects one connected RCP and starts its `otbr-agent` runtime, then listens on
IPv6. It remains Wi-Fi-only when no RCP is connected. Make Thread mandatory
with `--thread-required`:

```bash
extrittio run --thread-required
```

When auto-discovery is ambiguous, choose the radio in **OpenThread Settings**;
the host-local choice is reused on later starts. The `--thread-rcp` and
`--thread-infra-interface` options remain available as startup overrides.
Provision a Thread device with an IPv6 Zenoh locator such as
`tls/[fdxx:...]:7447`. The backend receives the same Extrittio Zenoh topics and
payloads as it does from Wi-Fi devices. See [OpenThread Edge deployment](../../docs/OPENTHREAD_EDGE.md)
for RCP firmware, runtime packaging, macOS/Linux configuration, and verification.

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

For the single-executable Turso Edge build, data layout and backup/restore commands, see [Turso Edge Deployment](../../docs/TURSO_EDGE.md).
