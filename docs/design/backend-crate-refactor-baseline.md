# Backend crate refactor implementation baseline

- **Status:** P0-A baseline
- **Captured:** 2026-08-31
- **Source revision:** `a75ff73c187e1ae9bae04fd9f1377e8662886242`
- **Host used for local observations:** Apple Silicon macOS (`aarch64-apple-darwin`)
- **Related plan:** [Backend crate architecture plan](backend-crate-architecture-plan.md)

## 1. Purpose and evidence rules

This document freezes the build, test, feature, dependency, and artifact baseline
before the backend is split. It is not a claim that every command is portable to
every developer machine, nor that the most recent remote CI run was green.

Evidence is labeled as follows:

- **Verified locally** means the command was run against this snapshot and exited
  successfully.
- **Defined in CI** means the command is present in the checked-in workflow, but
  was not rerun locally unless separately marked verified.
- **Not verified locally** means a prerequisite was absent or the check is
  platform-specific. It must not be interpreted as a failure.

Product sources under `apps/extrittio`, `crates/backend`, `crates/common`,
`crates/device-contract`, `crates/openthread-runtime`, and `crates/rule-engine`
matched the source revision while artifact measurements were taken. P0 work on
documentation, CI, and `xtask` was in progress concurrently and is not part of
the pre-refactor topology described here.

## 2. Toolchain and runtime prerequisites

The workspace pins Rust 1.90.0 with `rustfmt` and Clippy and uses Cargo resolver
2 with Rust edition 2024. The local observations used:

| Tool or service | Required by | Baseline |
| --- | --- | --- |
| Rust/Cargo | all Rust checks | `rustc 1.90.0`, `cargo 1.90.0`; verified locally |
| Protobuf compiler | `extrittio-common` build and CI | `libprotoc 34.1`; verified locally |
| PostgreSQL client headers/libraries | default/PostgreSQL builds | Homebrew libpq `18.3` was installed at `/opt/homebrew/opt/libpq`; `pg_config` was not on the interactive `PATH` |
| PostgreSQL server | PostgreSQL integration tests | CI uses PostgreSQL 17; no local server was listening on `127.0.0.1:5432` during capture |
| Docker | local PostgreSQL and packaging | required by the documented setup and Docker/Edge packaging; not exercised for this baseline |
| Node/npm | frontend and embedded Edge UI | Node 22 or newer and npm 10.9.3 are declared; `apps/frontend/dist` must exist before an `edge` build |
| Linux `ldd` | deployment linkage assertion | used in CI; unavailable as the native linkage tool on macOS |
| macOS `otool` | local linkage inspection | verified locally; it emitted sandbox cache warnings but returned the dependency list |

`cargo xtask doctor` is the repository prerequisite check. On macOS it discovers
Homebrew libpq at `/opt/homebrew/opt/libpq/bin/pg_config` even when that directory
is not on `PATH`. Linux CI installs `protobuf-compiler`, `libpq-dev`, and
`libssl-dev`; the PostgreSQL test job also installs `postgresql-client`.

To start the disposable development database expected by the integration tests:

```bash
docker compose -f deploy/docker/docker-compose.yml up -d postgres
```

The default test URL is:

```text
postgres://extrittio:extrittio@127.0.0.1:5432/extrittio?connect_timeout=2
```

`DATABASE_URL` overrides it. The tests run migrations and truncate product
tables, so the URL must point at a disposable test database.

## 3. Current workspace and package topology

`cargo metadata --locked --format-version 1 --no-deps` completed locally and
reported 13 workspace packages:

```text
extrittio
extrittio-backend
extrittio-client
extrittio-client-runtime
extrittio-common
extrittio-device-contract
extrittio-macos
extrittio-openthread-runtime
extrittio-rpi
extrittio-rule-engine
extrittio-sdk
extrittio-simulator
xtask
```

There is currently one backend package. `extrittio-backend-core`,
`extrittio-backend-postgres`, `extrittio-backend-turso`, and the adapter contract
test package do not yet exist.

### 3.1 Current ownership

```text
apps/extrittio
  -> extrittio-backend (default features disabled; app forwards a database feature)
  -> extrittio-openthread-runtime (direct dependency)

extrittio-backend
  -> extrittio-common
  -> extrittio-device-contract
  -> extrittio-openthread-runtime
  -> extrittio-rule-engine
  -> Diesel/PostgreSQL when `postgres` is active
  -> Turso when `turso` is active
  -> HTTP, Zenoh, object storage, certificates, workers, and observability libraries
```

The backend package owns both a library and an implicit
`src/main.rs` executable. It also declares an `openapi` binary. The application
package owns the user-facing `extrittio` executable. Removing the duplicate
backend process and the app's direct OpenThread dependency is P5.3 work.

### 3.2 Feature topology

| Package | Feature | Current expansion |
| --- | --- | --- |
| `extrittio-backend` | `default` | `postgres` |
| `extrittio-backend` | `postgres` | optional `diesel`, `diesel_migrations` |
| `extrittio-backend` | `turso` | optional `turso` 0.7.2 |
| `extrittio-backend` | `production` | `postgres`, `jemalloc`, `s3`, `otlp`, `swagger-ui`, `mdns` |
| `extrittio-backend` | `all-databases` | `postgres`, `turso` |
| `extrittio-backend` | `phase0-turso` | `turso`; enables the phase-0 integration target |
| `extrittio-backend` | `openapi` | no additional dependency |
| `extrittio-backend` | `swagger-ui` | `openapi`, vendored Swagger UI |
| `extrittio-backend` | `embedded-ui` | `include_dir`; build requires `apps/frontend/dist/index.html` |
| `extrittio` | `default` | `postgres` |
| `extrittio` | `production` | app `postgres`, app `jemalloc`, backend `production` |
| `extrittio` | `edge` | `turso`, backend `embedded-ui`, `swagger-ui`, `mdns` |
| `extrittio` | `all-databases` | `postgres`, `turso` |

The application depends on `extrittio-backend` with its default features
disabled, so selecting an app profile determines the backend adapter. The
backend itself still defaults to PostgreSQL for direct package commands.

The backend library currently contains this compile-time gate:

```text
enable at least one database backend feature: `postgres` or `turso`
```

Consequences at baseline:

- a database-free host/OpenAPI build is impossible;
- `all-databases` is declared but is not a CI lane;
- there is no compile-time rejection of selecting production and edge
  capabilities together;
- `extrittio-common` defaults to `std`, which enables Prost, JSON, and the Prost
  build dependency. Its `alloc` feature is separately checked by protocol CI.

## 4. Build and test commands

### 4.1 Workspace/PostgreSQL lane

The reproducible compile-only baseline is:

```bash
cargo check --locked --workspace --exclude extrittio-macos \
  --features extrittio-backend/openapi
```

**Verified locally:** passed. A first attempt overlapped an incomplete P0
`xtask` edit and failed because `mod architecture` temporarily had no file; the
same command passed after that parallel edit became complete. This transient is
not a pre-existing product failure.

The canonical backend verification entry point is:

```bash
cargo xtask verify backend
```

At the captured revision it expands to:

```bash
cargo fmt --all -- --check
cargo clippy --workspace --exclude extrittio-macos --all-targets -- -D warnings
cargo test --workspace --exclude extrittio-macos \
  --features extrittio-backend/openapi
cargo test -p extrittio-backend --no-default-features --features turso --lib
cargo run -p extrittio-backend --features openapi --bin openapi -- <temporary-path>
```

The generated OpenAPI file is byte-compared with `api/openapi.json` by `xtask`.
The CI job writes directly to `api/openapi.json` and then runs
`git diff --exit-code -- api/openapi.json`.

**Defined in CI, not fully verified locally:** the workspace Clippy and test
commands. The full test command requires a live PostgreSQL server. Port 5432
returned connection refused during capture, so the destructive PostgreSQL
integration suite was intentionally not run.

This narrower default-feature unit command does not perform database I/O:

```bash
cargo test --locked -p extrittio-backend --lib --features openapi
```

**Verified locally:** 59 passed, 0 failed, 0 ignored. This proves the
PostgreSQL/default feature compilation and backend unit layer only; it does not
replace the PostgreSQL integration lane.

CI supplies PostgreSQL 17 and runs:

```bash
DATABASE_URL='postgres://extrittio:extrittio@127.0.0.1:5432/extrittio?connect_timeout=2' \
  cargo test --workspace --exclude extrittio-macos \
  --features extrittio-backend/openapi
```

The workflow command does not currently use `--locked`.

### 4.2 Turso and Edge lane

CI verifies the adapter independently from the full Edge capability bundle:

```bash
cargo clippy -p extrittio --no-default-features --features turso \
  --all-targets -- -D warnings
cargo test -p extrittio-backend --no-default-features --features turso --lib
```

Locked equivalents were run locally:

```bash
cargo clippy --locked -p extrittio --no-default-features --features turso \
  --all-targets -- -D warnings
cargo test --locked -p extrittio-backend --no-default-features \
  --features turso --lib
```

**Verified locally:** Clippy passed; the library suite passed 67 tests with 0
failures and 0 ignored.

The dedicated engine/durability integration target is separate:

```bash
cargo test --locked -p extrittio-backend --no-default-features \
  --features phase0-turso --test turso_phase0
```

**Verified locally:** 3 passed and 3 ignored. Two ignored cases are subprocess
helpers invoked by the non-ignored process-lock/crash tests. The remaining
ignored case is the manual release-mode throughput probe. This target is not
run by current CI.

The Edge artifact path first requires generated frontend assets:

```bash
cd apps/frontend
npm ci
npm run build
cd ../..

cargo build --locked --profile ci-release -p extrittio \
  --no-default-features --features edge
```

**Verified locally:** the Rust build passed with an existing 4.6 MiB
`apps/frontend/dist`. Frontend generation itself was not rerun as part of this
baseline.

CI then performs a first-run smoke test equivalent to:

```bash
edge_binary=target/ci-release/extrittio
smoke_dir="$(mktemp -d)"
"$edge_binary" -o json migrate --database-backend turso \
  --deployment-profile edge --data-dir "$smoke_dir"
"$edge_binary" -o json database --database-backend turso \
  --deployment-profile edge --data-dir "$smoke_dir" info
"$edge_binary" run --help
```

**Verified locally:** all three commands exited 0. The new database reported
schema version 7 and `integrity: ok`.

### 4.3 Protocol and non-backend checks

Backend changes can affect shared device contracts. The current protocol lane is:

```bash
cargo xtask verify protocol
```

It checks `extrittio-common` and `extrittio-sdk` with `alloc`, tests the shared
Rust protocol/client crates, verifies committed nanopb bindings, and configures,
builds, and tests the C SDK with CMake/Ninja. CI installs nanopb 0.4.9.1.

This lane and frontend/iOS verification were not rerun for this baseline. They
remain required when a refactor changes their inputs.

## 5. Dependency-closure baseline

Use normal, non-dev edges for the deployable application closure:

```bash
cargo tree --locked -p extrittio --no-default-features \
  --features production -e normal --prefix none

cargo tree --locked -p extrittio --no-default-features \
  --features edge -e normal --prefix none
```

To reproduce the focused inspection used during capture:

```bash
cargo tree --locked -p extrittio --no-default-features \
  --features production -e normal --prefix none \
  | sort -u \
  | rg '^(diesel|diesel_migrations|pq-sys|turso|libsql|include_dir|tikv-jemallocator|utoipa-swagger-ui) '

cargo tree --locked -p extrittio --no-default-features \
  --features edge -e normal --prefix none \
  | sort -u \
  | rg '^(diesel|diesel_migrations|pq-sys|turso|libsql|include_dir|tikv-jemallocator|utoipa-swagger-ui) '
```

**Verified locally** on the host target:

| Profile | Relevant packages present | Relevant packages absent |
| --- | --- | --- |
| production | Diesel 2.3.11, Diesel migrations 2.3.2, `pq-sys` 0.7.5, jemalloc, Swagger UI | Turso, `include_dir` |
| edge | Turso 0.7.2, `include_dir` 0.7.4, Swagger UI | Diesel, Diesel migrations, `pq-sys`, jemalloc |

The manifest requests Diesel `2.2` with a compatible semver range; the locked
snapshot resolves 2.3.11. Turso is pinned exactly to 0.7.2.

These observations cover normal dependencies for the active macOS target. They
do not replace target-aware Linux checks, and a filtered display is not itself
a failing assertion. P0-C must turn the forbidden-package rules into explicit
machine checks. In particular, dynamic linkage alone cannot prove that an
unwanted statically linked dependency is absent.

## 6. Release artifact and linkage baseline

The two Cargo profiles used by CI are:

- `ci-release`: thin LTO and 16 codegen units; used for ordinary main-branch
  Docker/Edge builds;
- `release`: fat LTO, one codegen unit, stripped symbols, and aborting panics;
  used for tagged artifacts.

Both database profiles produce the same output path, so build and measure one
profile before building the other.

### 6.1 Reproduction procedure

Production:

```bash
cargo build --locked --profile ci-release -p extrittio \
  --no-default-features --features production
```

Edge, after generating `apps/frontend/dist`:

```bash
cargo build --locked --profile ci-release -p extrittio \
  --no-default-features --features edge
```

Linux measurement and linkage:

```bash
wc -c < target/ci-release/extrittio
file target/ci-release/extrittio
ldd target/ci-release/extrittio
sha256sum target/ci-release/extrittio
```

macOS measurement and linkage:

```bash
stat -f '%z bytes' target/ci-release/extrittio
file target/ci-release/extrittio
otool -L target/ci-release/extrittio
shasum -a 256 target/ci-release/extrittio
```

Repeat with `--profile release` and `target/release` for tagged-release numbers.
Size comparisons are meaningful only for the same OS, architecture, profile,
toolchain, lockfile, and frontend asset set.

### 6.2 Fresh local measurements

| Build | Size | SHA-256 | Linkage observation |
| --- | ---: | --- | --- |
| `ci-release`, production, arm64 macOS | 36,433,920 bytes (34.75 MiB) | `de26883ab746fdce08dad0c633994e1e6657cbdd09b221c175d9e02ff9b2e4da` | links `/opt/homebrew/opt/libpq/lib/libpq.5.dylib`; no Turso package in the normal Cargo closure |
| `ci-release`, edge, arm64 macOS | 48,843,264 bytes (46.58 MiB) | `320c0e36c310c87539c53fc22cfdcdad1cc5e85bdbb90ba63fc731d85cb10ba7` | no libpq/PostgreSQL dynamic library; no Diesel or `pq-sys` in the normal Cargo closure |

Both builds completed locally. These hashes are diagnostic snapshot evidence,
not permanent golden values. The Edge artifact embeds the current frontend, so
frontend output changes legitimately change its size and hash.

Current Linux Edge CI runs `ldd` and fails if a line matches
`libpq|postgres`, then uploads amd64 and arm64 executables with SHA-256 files.
The production Docker runtime explicitly installs `libpq5`. There is no current
CI size budget, size-delta report, production linkage assertion, or standalone
production binary artifact.

## 7. Test inventory and isolation requirements

The following counts are source annotations, not the number selected by every
feature combination:

| Area | `#[test]`/`#[tokio::test]` annotations |
| --- | ---: |
| backend unit sources | 72 |
| backend integration targets | 41 |
| `apps/extrittio` unit sources | 6 |
| common | 11 |
| device contract | 7 |
| rule engine | 61 |
| OpenThread runtime | 51 |
| `xtask` | 14 |

Backend integration targets are currently:

| Target | Feature gate | Cases | Requirements and behavior |
| --- | --- | ---: | --- |
| `api_tests.rs` | `postgres` | 24 | live disposable PostgreSQL; runs migrations; truncates and reseeds tables; opens an in-process Zenoh session |
| `cert_tests.rs` | `postgres` | 11 | same disposable PostgreSQL behavior; exercises certificate generation/storage and HTTP flows |
| `turso_phase0.rs` | `phase0-turso` | 6 | temporary local files only; 3 normal tests, 2 ignored subprocess helpers, 1 ignored manual throughput probe |

Each PostgreSQL integration target uses a process-local mutex and a pool of size
one. The two files use separate locks but the same default database. The test
database must never contain valuable data, and future test extraction should
provide stronger per-suite isolation rather than preserving the shared mutable
database as an architectural contract.

The ordinary Turso CI command uses `--lib`; it exercises Turso unit/adapter tests
inside the library but does not select `turso_phase0.rs`. The current test layout
also does not run one shared semantic suite against both adapters. That missing
contract suite is a primary reason for the refactor plan's test-only package.

## 8. CI coverage at the captured revision

The main workflow ignores `docs/**` and all Markdown-only changes. A P0-A
documentation-only change therefore receives no CI run unless another changed
path selects the workflow.

| Job | Current coverage | Important boundary |
| --- | --- | --- |
| Dependency Review | changed dependencies on pull requests | PR only |
| Dependency Audit | RustSec audit | no product build |
| Dependency Policy | `cargo deny check` | no feature-closure assertion |
| Backend Format | workspace rustfmt | formatting only |
| Backend Clippy | default-feature workspace, all targets, excluding `extrittio-macos` | PostgreSQL/default profile only; not explicit production/edge/all-databases |
| Backend Test | PostgreSQL 17 workspace tests plus OpenAPI generation/diff | requires shared database; no adapter-neutral OpenAPI build |
| Turso Edge | amd64 and arm64 Linux Turso Clippy/lib tests, Edge build, linkage check, migration/info/help smoke, checksum artifact | omits `turso_phase0` integration target; Clippy uses `turso`, while artifact uses full `edge` |
| Clients & Protocol | Rust/C/nanopb contract verification | separate from backend adapter semantics |
| Frontend | format, lint, tests, generated API types, build | produces assets consumed by Edge job |
| Docker | production image, SBOM, signature, provenance | push to main/tags only; not a pull-request production build |

The Docker workflow builds the `production` feature with PostgreSQL for amd64
and, on tags, arm64. Main uses `ci-release`; tags use `release`. Edge CI similarly
uses `ci-release` except on tags.

There is no current CI lane for:

- a backend host with no database adapter;
- both adapters enabled together;
- a production dependency-closure assertion that rejects Turso;
- an edge dependency-closure assertion that rejects Diesel beyond the Linux
  dynamic-link test;
- the `phase0-turso` integration target;
- the future core, PostgreSQL adapter, Turso adapter, or shared contract-test
  packages;
- release-size regression reporting.

## 9. Known baseline gaps and owners

These are baseline gaps, not accepted permanent exceptions:

| Gap | Planned owner |
| --- | --- |
| No executable architecture/dependency-edge policy | P0.4/P0-C, fatal package rules in P2.5 |
| Documentation-only changes skip CI | P0-C CI policy decision |
| No machine assertion for production excluding Turso | P0-C initially; P2.5 and P5.4 for final package graph |
| Edge exclusion relies partly on `ldd` | P0-C Cargo-closure assertion; P2.5 final graph |
| No database-free host/OpenAPI build | P2.3 foundations, P2.5 matrix, P5.4 normalization |
| No both-adapter CI lane | activate in P2.5 after the packages exist |
| No shared adapter semantic suite | P2.4 pilot and Section 18; expanded throughout P3 |
| `turso_phase0` integration tests are not in CI | P0-C current-profile CI |
| PostgreSQL integration tests share and destructively reset one database | P0-B characterization, then adapter harness work in P2/P3 |
| OpenAPI, CLI, wire, outbox, backup, object-key, and encryption compatibility are not comprehensively golden-tested | P0.2/P0-B |
| One package owns host, domain, both adapters, and a duplicate executable | P1 boundaries, P2 extraction, P5.3 cleanup |
| Backend cannot compile without a database feature | P2.3/P2.5 |
| `all-databases` exists but is not continuously checked | P2.5 and P5.4 |
| No release size threshold or before/after report | P6 release verification |
| Production artifact build happens only after merge/tag through Docker | P0-C should add closure checks; P6 decides whether a PR artifact build is warranted |

No pre-existing product-code failure was established during this capture. The
full PostgreSQL lane is simply **unverified locally** because its service
prerequisite was absent. Remote workflow status was not queried.

## 10. Refactor comparison rules

At each crate-migration milestone:

1. Run the workspace/PostgreSQL lane against a disposable PostgreSQL 17
   database and record the result.
2. Run the Turso library suite and `phase0-turso` integration target.
3. Run the applicable protocol/OpenAPI compatibility fixtures.
4. Inspect both normal dependency closures and make forbidden packages a hard
   failure rather than visually checking filtered output.
5. Build production and Edge with the same OS, architecture, Cargo profile,
   toolchain, lockfile, and frontend assets used for the comparison.
6. Record byte size and linkage for each artifact. Explain meaningful growth;
   do not fail solely on cross-platform size differences.
7. Run the Edge migration/info/help smoke test in a fresh temporary directory.
8. Never add a new failure to this document as a way to normalize a regression.

The P6 release comparison should use Linux amd64 and arm64 artifacts because
those are the deployed targets. The macOS measurements above are a useful local
reference, not the release acceptance threshold.
