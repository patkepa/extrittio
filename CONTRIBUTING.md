# Contributing to Extrittio

Thank you for improving Extrittio. Changes should be reviewable, tested, and
compatible with the repository's tenant-isolation and device-protocol guarantees.

## Before You Start

- Use an issue for substantial features, schema changes, or protocol changes so
  maintainers can align on scope first.
- Never include credentials, production data, private keys, or customer device
  identifiers in code, fixtures, logs, screenshots, or commits.
- Treat every backend query and event as tenant-scoped unless it is explicitly a
  platform-wide operation.

## Development Setup

The supported toolchains are declared in `rust-toolchain.toml` and
`apps/frontend/package.json`. Start PostgreSQL and install frontend dependencies:

```bash
docker compose -f deploy/docker/docker-compose.yml up -d postgres
cargo run -p extrittio -- migrate
cd apps/frontend && npm ci
```

GitHub Packages dependencies under `@extrittio` require a package token in
`NODE_AUTH_TOKEN` with read access.

## Quality Gates

Run the checks relevant to your change before opening a pull request:

```bash
cargo fmt --all -- --check
cargo clippy --workspace --exclude extrittio-macos --all-targets -- -D warnings
cargo test --workspace --exclude extrittio-macos

cd apps/frontend
npm run format:check
npm run lint
npm test
npm run build
```

Changes to protobuf definitions must regenerate and commit the C nanopb output:

```bash
clients/c/sdk-c/scripts/generate-proto.sh
```

Changes to REST routes or schemas must regenerate and commit both API artifacts:

```bash
cd apps/frontend && npm run generate-api
```

## Database and API Changes

- Add forward and rollback SQL in a new Diesel migration. Do not edit an applied
  migration or `crates/backend/src/db/schema.rs` by hand.
- Preserve tenant-qualified foreign keys and repository method signatures.
- Prefer additive, backward-compatible API and protobuf evolution.
- Document operational consequences such as backfills, retention, downtime, and
  rollback limitations in the pull request.

## Pull Requests

Keep commits focused, explain the user-visible result, include verification
evidence, and call out security, migration, compatibility, and deployment impact.
At least one maintainer review is required; sensitive authentication, tenancy,
cryptography, migration, and release changes should receive two reviews.

By participating, you agree to follow [CODE_OF_CONDUCT.md](CODE_OF_CONDUCT.md).
