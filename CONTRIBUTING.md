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
cargo xtask verify --changed
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
- Add PostgreSQL migrations under `crates/backend/migrations/postgres` and the
  matching Turso migration under `crates/backend/migrations/turso` when a
  feature supports both backends.
- Do not edit an already-applied migration. Regenerate Diesel schema output
  through the established migration workflow instead of hand-editing it.
- Prefer additive API and device-protocol evolution. Explain unavoidable
  compatibility or rollback limits in the pull request.
- Keep HTTP parsing in route modules, policy and orchestration in domain
  services, and database-specific work in persistence adapters.

## Pull requests

Explain the user-visible result, include verification evidence, and call out
security, tenancy, migration, API, device-contract, and deployment impact.
Substantial architecture or protocol changes should start with a focused design
proposal before implementation.
