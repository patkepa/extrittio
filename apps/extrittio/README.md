# Extrittio server and CLI

The `extrittio` package provides the PostgreSQL server, the single-node Edge
runtime, and administrative HTTP client commands.

## Run from source

From the repository root:

```bash
cargo xtask cloud run dev
cargo xtask edge run dev
```

These commands supervise the backend and Vite development server together.
For direct CLI and service invocation:

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

The `edge-runtime` feature provides the local Turso runtime used during source
development. The `edge` feature adds the embedded web console for deployable
artifacts:

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

The web console and `api/openapi.json` are currently the supported interfaces
for creating devices from published blueprint revisions. The CLI's legacy
`devices create` and `provision` arguments still use the compatibility
`device_type_id` request shape and should not be used for new blueprint-based
provisioning.

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
