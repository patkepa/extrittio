# Contributing to Extrittio

Keep changes reviewable, tenant-safe, and compatible with the device contract
and public API.

## Setup

The supported Rust and Node.js versions are declared in
`rust-toolchain.toml` and `apps/frontend/package.json`. Check the full local
toolchain with:

```bash
cargo xtask doctor
```

For backend and frontend development:

```bash
docker compose -f deploy/docker/docker-compose.yml up -d postgres
cargo run -p extrittio -- migrate

cd apps/frontend
npm ci
```

Never commit credentials, private keys, customer data, production database
content, or real device identities.

## Verification

Prefer the repository tasks because they match CI:

```bash
cargo xtask verify backend
cargo xtask verify frontend
cargo xtask verify protocol
cargo xtask verify ios
```

The `all` scope includes iOS and needs Xcode plus the tools listed in
`apps/mobile-app-ios/Tools/versions.env`.

## Generated contracts

Changes to Protobuf definitions must regenerate and commit the C bindings:

```bash
cargo xtask protocol generate
```

Changes to REST routes or schemas must regenerate both API artifacts:

```bash
cd apps/frontend
npm run generate-api
```

Commit `api/openapi.json` and `apps/frontend/src/types/openapi.ts` together.

## Backend changes

- Every tenant-owned query, event, and foreign-key relationship must remain
  tenant-scoped.
- Add PostgreSQL migrations under `crates/backend-postgres/migrations` and the
  matching Turso migration under `crates/backend-turso/migrations` when a
  feature supports both backends.
- Do not edit an already-applied migration. Regenerate Diesel schema output
  through the established migration workflow instead of hand-editing it.
- Prefer additive API and device-protocol evolution. Explain unavoidable
  compatibility or rollback limits in the pull request.
- Keep HTTP/Zenoh parsing and runtime scheduling in `crates/backend`, policy
  and business orchestration in `crates/backend-core`, and SQL, rows, migrations,
  and database lifecycle operations in the PostgreSQL/Turso adapter crates.
- `apps/extrittio` owns the service executable (`extrittio serve` / `extrittio run`).
  The host crate is a library with an explicit `openapi` generator; it has no
  standalone service binary. Native allocator selection belongs to the CLI.
- Run `cargo xtask architecture` to check package boundaries and production/edge
  dependency isolation. The current refactor's test deferral and outstanding
  verification are tracked in [the execution plan](docs/design/backend-refactor-execution-plan.md).

## Pull requests

Explain the user-visible result, include verification evidence, and call out
security, tenancy, migration, API, device-contract, and deployment impact.
Substantial architecture or protocol changes should start with a focused design
proposal before implementation.
