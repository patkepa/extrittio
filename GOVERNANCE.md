# Project Governance

Extrittio uses a maintainer-led, review-based governance model.

## Roles

- **Contributors** propose issues, code, documentation, and reviews.
- **Maintainers** merge changes, triage releases and security reports, and uphold
  compatibility, quality, and conduct standards.
- **Repository owners** appoint or remove maintainers, resolve deadlocks, and
  administer credentials, protected branches, and releases.

Maintainer status is earned through sustained, technically sound contributions,
constructive review, and reliable stewardship. An owner nominates a candidate and
the existing owners decide by consensus. Inactive maintainers may move to emeritus
status after notice.

## Decisions

Routine changes use pull-request review. Significant architecture, protocol,
security, data-model, or governance changes begin with a repository design issue
or plan and allow reasonable time for feedback. Maintainers seek consensus; if
none emerges, repository owners make and document the decision.

At least one maintainer approval is required to merge. Two approvals are expected
for authentication, authorization, tenant isolation, cryptography, irreversible
migrations, release infrastructure, and governance changes. Authors do not
self-approve those sensitive changes.

## Releases and Security

Repository owners control releases. A release must pass protected CI, use an
immutable version tag, and publish signed image/provenance/SBOM artifacts. Security
reports follow [SECURITY.md](SECURITY.md) and may be handled privately until a fix
and coordinated disclosure are ready.

Governance changes use the same review process and are recorded in this file.
The project is private and proprietary; granting a source license or changing
that status requires explicit repository-owner approval.
