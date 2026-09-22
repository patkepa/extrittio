# Security Policy

## Supported Versions

Security fixes are applied to the latest released minor line. During the current
pre-1.0 phase, only the latest published release is supported.

## Reporting a Vulnerability

Do not open a public issue for a suspected vulnerability. Use GitHub's private
vulnerability reporting for the `patkepa/extrittio` repository, or privately
contact a repository owner if that feature is unavailable.

Include the affected version or commit, impact, reproduction steps, and any known
mitigations. Remove real credentials, customer data, and device identities from
the report.

Maintainers aim to acknowledge a report within three business days, provide an
initial assessment within seven business days, and coordinate disclosure after a
fix is available. Timelines may change with severity and complexity; reporters
will be kept informed.

## Security Expectations

- Production deployments must replace all example secrets, enable secure cookies
  and Zenoh TLS, restrict database/network access, and use a trusted TLS ingress.
- Release images are published by digest with an SBOM, keyless signature, and
  build-provenance attestation.
- Dependency changes are reviewed and audited in CI.
