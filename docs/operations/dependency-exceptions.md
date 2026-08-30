# Dependency policy exceptions

Dependency exceptions are temporary, reviewed security decisions. Each entry
records why the affected code is acceptable, the condition that removes the
exception, and its next review date. The advisory IDs in this file must match
the `ignore` list in `deny.toml`.

## Active exceptions

| Advisory | Dependency path | Current exposure | Removal condition | Review by |
| --- | --- | --- | --- | --- |
| `RUSTSEC-2026-0041` | `zenoh-transport -> lz4_flex 0.10` | `lz4_flex` is present in the build, but Zenoh calls the affected block decompressor only in code gated by `transport_compression`. Workspace Zenoh dependencies disable default features and do not enable that feature. | Remove when Zenoh uses a RustSec-patched version (`>=0.11.6,<0.12.0` or `>=0.12.1`), or immediately if transport compression is enabled. | 2026-11-04 |
| `RUSTSEC-2024-0436` | `zenoh-keyexpr -> token-cell -> paste` | The advisory reports an unmaintained dependency rather than a known vulnerability. It is an upstream macro dependency and is not used directly by Extrittio. | Remove when Zenoh or `token-cell` migrates to a maintained macro implementation. | 2026-11-04 |
| `RUSTSEC-2025-0134` | `zenoh-link-tls -> rustls-pemfile` | The advisory reports an unmaintained dependency rather than a known vulnerability. Version 2.2 is a wrapper over the maintained PEM parser in `rustls-pki-types`. | Remove when Zenoh consumes `rustls-pki-types::pem` directly. | 2026-11-04 |

## Review procedure

At each review date:

1. update the lockfile;
2. run `cargo deny check`;
3. inspect the relevant features with `cargo tree -e features`;
4. remove the exception or record new owner-approved evidence and a review
   date.

Do not add a bare advisory ID to `deny.toml`. Document reachability, mitigation,
removal criteria, and review ownership here first.
