# `extrittio-backend-postgres`

PostgreSQL persistence adapter for `extrittio-backend-core`.

The P2 walking skeleton owns the cloneable Diesel/r2d2 executor boundary and
implements the tenant-scoped zone CRUD and system-scoped rule-zone snapshot
ports. Zone lists use bytewise PostgreSQL `C` collation with the canonical
`name, id` ordering; snapshots use `tenant_id, name, id`.

PostgreSQL migrations and Diesel schema/model generation are owned by this
crate. The host temporarily re-exports the adapter models while the remaining
repository slices migrate; it does not own a second migration chain.

Local verification (no database required):

```sh
cargo check -p extrittio-backend-postgres --offline
cargo test -p extrittio-backend-postgres --offline
cargo clippy -p extrittio-backend-postgres --all-targets --offline -- -D warnings
```

The adapter compiles and its SQL/translation/migration-embedding unit tests run
without PostgreSQL. The shared contract suite runs in CI against the disposable
PostgreSQL service; locally it runs when `DATABASE_URL` is set.
