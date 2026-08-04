# Dependency Policy Exceptions

Dependency exceptions are temporary, reviewed security decisions rather than a
way to make CI green. Each entry records why the affected code is acceptable,
the condition that removes the exception, and its next review date.

## Active Exceptions

| Advisory | Dependency path | Current exposure | Removal condition | Review by |
| --- | --- | --- | --- | --- |
| `RUSTSEC-2026-0041` | `zenoh-transport -> lz4_flex 0.10` | The vulnerable block decompressor is compiled only by Zenoh's `transport_compression` feature. Every workspace Zenoh dependency disables default features and enables only TCP, TLS, and, for the backend, `unstable`; `transport_compression` is not enabled. | Remove when Zenoh accepts `lz4_flex >=0.11.6`, or immediately if transport compression is enabled. | 2026-11-04 |
| `RUSTSEC-2024-0436` | `zenoh-keyexpr -> token-cell -> paste` | `paste` is unmaintained, but the advisory reports no vulnerability. It is an upstream macro dependency and is not used directly by Extrittio. | Remove when Zenoh/token-cell migrates to a maintained macro implementation. | 2026-11-04 |
| `RUSTSEC-2025-0134` | `zenoh-link-tls -> rustls-pemfile` | `rustls-pemfile` is unmaintained, but the advisory reports no vulnerability; version 2.2 is a thin wrapper over the maintained PEM parser in `rustls-pki-types`. TLS remains enabled and required. | Remove when Zenoh consumes `rustls-pki-types::pem` directly. | 2026-11-04 |

## Review Procedure

At each review date, update the lockfile, run `cargo deny check`, verify the
feature graph with `cargo tree -e features`, and either remove the exception or
record a new owner-approved review date with updated evidence. New exceptions
must document reachability and mitigations here before their advisory ID is
added to `deny.toml`.
