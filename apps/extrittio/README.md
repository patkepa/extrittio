# Extrittio server and CLI

The `extrittio` package provides the PostgreSQL server, the single-node Edge
runtime, and administrative HTTP client commands.

## Run from source

From the repository root:

```bash
cargo run -p extrittio -- --help
cargo run -p extrittio -- serve
cargo run -p extrittio -- migrate
cargo run -p extrittio -- init
```

No subcommand defaults to `serve`. That command initializes the configured
database, opens Zenoh, starts background workers, and serves the REST API. It
also serves `apps/frontend/dist` when present. Use `--ui-dir` to choose another
build or `--no-ui` for API-only operation.

`migrate` applies pending migrations and exits. `init` also seeds built-in
records, creates the initial owner when needed, and creates service
certificates.

## Edge runtime

The `edge` feature embeds the built web console and selects the local Turso
profile for `extrittio run`:

```bash
cd apps/frontend
npm ci
npm run build
cd ../..

cargo install --path apps/extrittio --locked \
  --no-default-features --features edge

EXTRITTIO_BOOTSTRAP_ADMIN_PASSWORD='choose-a-strong-password' extrittio run
```

Use `cargo xtask install edge --release` when the host installation should also
build and install the pinned OpenThread Border Router agent. Packaged workflows,
data layout, and backup/restore commands are documented in
[Edge deployment](../../docs/deployment/edge.md).

## Administrative client

The executable also exposes authentication, health, device, fleet, firmware,
OTA, API-key, and certificate command groups. Use generated help as the current
command reference:

```bash
extrittio --help
extrittio auth --help
extrittio database --help
extrittio devices --help
```

Create devices from an immutable published blueprint revision:

```bash
extrittio devices create --name workshop-sensor --blueprint-revision-id REVISION_ID
```

Device creation and `provision` no longer accept device-type selectors, and the
`device-types` command has been removed. Blueprint publishing remains available
through the web console and HTTP API. To create a device and save its verified
contract response for a native client:

```bash
extrittio provision --name workshop-sensor --blueprint-revision-id REVISION_ID \
  --contract-out ./device-contract.json
```

The destination must not exist and its parent directory must be writable.
Provisioning verifies contract hash, device identity and revision before saving
the file with private permissions on Unix. The endpoint is taken from the
contract, not a CLI default. If contract retrieval or later certificate/NVS work
fails after creation, retain the created device ID instead of blindly creating
another device. The ESP NVS format itself still needs contract-native migration.

Firmware upload and filtering also use published revisions:

```bash
extrittio firmware upload --blueprint-revision-id REVISION_ID --file firmware.bin
extrittio firmware list --blueprint-revision-id REVISION_ID
```

Firmware commands reject `--device-type-id`. Their output reports revision
identity; JSON output also includes compatibility metadata and update strategy.

Browser sessions use HTTP-only cookies. CLI commands can use a bearer token
from `--token`, `EXTRITTIO_TOKEN`, or the saved login session. Override the
server with `--url` or `EXTRITTIO_URL`, and use `--output json` for automation.
The default CLI configuration path is the operating system's Extrittio config
directory; override it with `--config` or `EXTRITTIO_CLI_CONFIG`.

## Source layout

- `args.rs` defines the clap command surface.
- `commands/` implements command families.
- `api.rs` owns authenticated HTTP transport.
- `models.rs` and `output.rs` own response models and table/JSON rendering.
- `commands/service.rs` composes the server and Edge runtimes.
- `esp32.rs` contains legacy ESP-IDF NVS provisioning support.

The backend domains and persistence adapters live in `crates/backend`; see the
[system architecture](../../docs/architecture/overview.md).
