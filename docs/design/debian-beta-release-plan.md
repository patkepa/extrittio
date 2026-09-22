# Debian beta release plan

## 1. Goal

Ship reproducible Extrittio Edge `arm64` Debian packages from version tags so a
Raspberry Pi 4 or 5 running a 64-bit Debian-family operating system can install
and upgrade Extrittio without compiling it.

The beta user journey is:

```text
maintainer pushes v0.2.0-beta.1
  -> CI validates the tag and source revision
  -> CI builds and tests one immutable .deb
  -> CI publishes the same .deb to a GitHub prerelease
  -> CI publishes the same .deb to the beta APT channel
  -> user installs or upgrades with apt
```

This plan treats GitHub Releases as the immutable release record and a signed
APT repository as the installation and update transport. Mutable channel
metadata may point to a newer package, but a published versioned package must
never be rebuilt or overwritten.

## 2. Beta scope and non-goals

### In scope

- Raspberry Pi 4 and 5 with a 64-bit OS (`arm64`).
- Debian 12 Bookworm and the corresponding 64-bit Raspberry Pi OS release.
- The existing embedded-UI, Turso-backed `extrittio run` binary.
- A systemd unit, persistent `/var/lib/extrittio` state, and the conffile at
  `/etc/extrittio/extrittio.env`.
- Versioned `.deb`, SHA-256 checksum, SBOM, and build-provenance attestation.
- GitHub prereleases and a signed `beta` APT channel.
- Explicit operator-controlled upgrades during beta.
- A documented manual rollback and restore procedure.

### Deferred

- `amd64`, Ubuntu-specific certification, Debian stable-repository inclusion,
  Raspberry Pi images, and bundled OTBR in the Debian package.
- Silent automatic application upgrades. During beta, notify and document;
  operators run `apt upgrade` deliberately.
- Full operating-system OTA and A/B root filesystems.
- Stable-channel publication and compatibility guarantees.
- Firmware/client OTA. This workflow distributes the Edge server only.

The complete Raspberry Pi archive and container image may continue to be built
by their existing workflows. They should share the release version and release
notes, but they must not block the first Debian beta unless explicitly selected
as release requirements.

## 3. Release and version policy

### 3.1 Tags

Beta releases use immutable annotated tags in this form:

```text
vMAJOR.MINOR.PATCH-beta.N
```

Example:

```text
v0.2.0-beta.1
```

Release candidates may later use `v0.2.0-rc.1`. A stable tag is
`v0.2.0`. Until the stable-readiness checklist in section 12 is complete, the
workflow rejects stable tags rather than accidentally populating a stable
channel.

### 3.2 Debian version mapping

SemVer prerelease separators must be converted so Debian sorts prereleases
before the final version:

| Git tag | Debian package version |
| --- | --- |
| `v0.2.0-beta.1` | `0.2.0~beta.1` |
| `v0.2.0-rc.1` | `0.2.0~rc.1` |
| `v0.2.0` | `0.2.0` |

The conversion is implemented once in `xtask` or a small checked-in release
script and covered by table-driven tests. Workflow shell snippets must not each
implement their own conversion.

### 3.3 Single version authority

For every release, CI must prove all of the following:

- The tag parses as supported SemVer.
- The tag's base/version agrees with `[workspace.package].version`, including
  the chosen prerelease policy.
- The binary's reported version, Debian `Version`, artifact filenames, SBOM,
  GitHub release, and APT metadata identify the same release.
- The tagged commit is reachable from `main`.
- No package with the same Debian version already exists in the target APT
  channel.

The implementation must decide whether the workspace manifest carries the full
prerelease version or only the release line. Prefer carrying the full version
in the manifest because `extrittio --version` then remains correct outside CI.

## 4. Artifact contract

Each beta tag produces:

```text
extrittio_<debian-version>_arm64.deb
extrittio_<debian-version>_arm64.deb.sha256
extrittio_<debian-version>_arm64.spdx.json
```

The `.deb` contains only:

- `/usr/bin/extrittio`
- `/usr/lib/systemd/system/extrittio.service`
- `/etc/extrittio/extrittio.env` as a Debian conffile
- Debian control and maintainer scripts
- copyright, license, and changelog files under `/usr/share/doc/extrittio/`

Required package properties:

- `Package: extrittio`
- `Architecture: arm64`
- dynamically calculated shared-library dependencies
- explicit supported-OS metadata in the release notes
- no PostgreSQL runtime linkage
- no embedded bootstrap password
- package upgrades never delete or replace `/var/lib/extrittio`
- upgrades preserve locally edited `/etc/extrittio/extrittio.env`
- install and upgrade do not unexpectedly start an unconfigured public service

The current builder is a useful base, but implementation must close these gaps:

- add Debian copyright/changelog metadata and run `lintian`;
- validate package version input rather than interpolating arbitrary text;
- make maintainer scripts handle install, upgrade, removal, and purge explicitly;
- decide and document service restart behavior on upgrade;
- ensure the package cannot start with the development `admin` password;
- expose build version, commit, and timestamp from the installed binary;
- test conffile and data-directory preservation across upgrades.

Root execution is currently intentional because supervised OTBR may create
`wpan0` and routes. The beta documentation must state this clearly. Reducing
privileges or splitting OTBR into a separate service is a later hardening task,
not a silent packaging change.

## 5. Repository and workflow layout

Create a reusable workflow instead of expanding the existing CI file with all
packaging details:

```text
.github/workflows/ci.yml
.github/workflows/debian-package.yml
.github/workflows/release.yml
deploy/debian/
tools/xtask/
scripts/                    # only if xtask is not appropriate
docs/deployment/debian.md
```

Responsibilities:

- `ci.yml`: ordinary source checks and the release gate.
- `debian-package.yml`: reusable build/test workflow with artifact output; it
  never publishes externally when invoked by a pull request.
- `release.yml`: tag validation, gated publication, attestations, GitHub
  prerelease creation, and APT upload.
- `xtask`: version parsing/mapping and deterministic package orchestration.
- `deploy/debian`: package payload and maintainer-script sources.

`debian-package.yml` should support `workflow_call` inputs for source version,
Debian version, architecture, build profile, and whether this is a release
build. Only `arm64` is accepted initially.

## 6. Pull-request validation

Changes to Cargo manifests/lockfiles, the frontend, Edge sources, `xtask`,
Debian packaging, or release workflows trigger a non-publishing package job.

The job performs:

1. Build frontend assets with `npm ci` and the locked Node version.
2. Build the Edge binary with `--locked`, `--release`, no default features,
   and the `edge` feature.
3. Build the package using the existing BuildKit-based packaging path.
4. Check `dpkg-deb --info` and `dpkg-deb --contents` against the artifact
   contract.
5. Run `lintian` with an explicit, documented allowlist for any temporary beta
   warnings. New warnings fail CI.
6. Verify executable architecture and dynamic linkage.
7. Start the unpacked/installed binary with Thread disabled in a temporary
   data directory, run migrations, query database info, and check `/health`.
8. Install the package in a clean Debian Bookworm `arm64` environment and
   verify paths, permissions, conffile registration, and systemd-unit syntax.
9. Upgrade from a checked-in/generated previous-package fixture or the latest
   published beta; prove configuration and seeded database state survive.
10. Remove the package and prove persistent data remains. Purge behavior must
    be separately specified and tested before it is allowed to delete config;
    persistent user data should still require an explicit operator action.

The first implementation can use QEMU/Buildx for a Debian container. Before
the first public beta, repeat the install, reboot, service, health, and upgrade
test on an actual Pi 4 and Pi 5; record the tested OS image versions in the
release checklist.

## 7. Tagged release workflow

`release.yml` runs only for `v*` tags and uses a concurrency group based on the
tag with cancellation disabled.

### 7.1 Validate

- Check out the exact tag with full history.
- Reject lightweight tags if annotated/signed tags are policy.
- Parse and validate the version and beta sequence.
- Reject a stable version while `ALLOW_STABLE_RELEASES` is not enabled in the
  protected release environment.
- Fetch `main` and prove the commit is reachable from it.
- Verify changelog/release-note content exists.
- Verify the target version is absent from GitHub Releases and the APT index.
- Run the complete repository release gate, not only changed-path jobs.

### 7.2 Build once

- Invoke the reusable Debian workflow with the production release profile.
- Generate the `.deb`, checksum, and SPDX SBOM.
- Upload them as short-lived Actions artifacts for subsequent jobs.
- Record SHA-256 digests in job outputs.

No later publication job may rebuild the package. GitHub and APT must receive
the bytes produced by this single gated build.

### 7.3 Test release artifacts

- Download the workflow artifact in a separate job.
- Recompute and verify checksums.
- Repeat package inspection and fresh-install smoke tests.
- Run the latest-beta-to-new-beta upgrade test when a previous beta exists.
- Produce a machine-readable test summary attached to the workflow run.

### 7.4 Approval and environment

Publishing uses a protected GitHub environment named `beta-release` with:

- required maintainer approval during the initial beta;
- the APT repository credential;
- no secrets exposed to pull-request builds;
- environment-scoped domain/CDN credentials if needed.

The approval occurs after artifacts and tests exist, so a reviewer approves
specific digests rather than an unbuilt tag.

### 7.5 Attest

Grant the smallest required workflow permissions:

```yaml
contents: write
id-token: write
attestations: write
```

Use GitHub's build-provenance action on the `.deb`, checksum, and SBOM. Also
attest the existing container by digest in its own workflow. Publish
verification commands in the release notes, for example:

```bash
gh attestation verify extrittio_0.2.0~beta.1_arm64.deb \
  --repo patkepa/extrittio
sha256sum --check extrittio_0.2.0~beta.1_arm64.deb.sha256
```

### 7.6 Publish GitHub prerelease

- Create a draft prerelease for beta tags.
- Attach the exact tested `.deb`, checksum, SBOM, and test summary.
- Generate notes from a checked-in changelog section, supplemented by GitHub's
  comparison notes.
- Mark supported OS/architecture, known limitations, migration notes, backup
  command, upgrade command, and rollback constraints prominently.
- Publish only after every upload succeeds.
- Enable GitHub immutable releases at the repository level. Because immutable
  assets cannot be replaced, any correction receives a new beta number.

### 7.7 Publish the beta APT channel

Use a hosted signed APT repository for the beta rather than designing repository
signing and highly available index hosting in the first iteration. Cloudsmith
is the initial recommendation; the workflow should isolate upload details in
one step so another provider or a self-hosted `reprepro` repository can replace
it later.

Repository coordinates should be stable and product-owned, for example:

```text
packages.extrittio.io/debian beta main
```

The hosted repository must:

- sign `InRelease`/`Release` metadata;
- serve packages over HTTPS;
- reject replacement of an existing version;
- retain old beta packages for rollback/testing;
- expose repository health and download logs without collecting device secrets;
- support key rotation with an overlap period.

Upload the already-attested `.deb`, wait for indexing, then verify from a clean
Bookworm environment that `apt-cache policy extrittio` selects the new beta and
that installation by exact version succeeds. Only after this verification does
the workflow publish the GitHub draft release. If APT publication fails, leave
the release in draft and do not claim success.

## 8. Installation and `get.extrittio.io`

During beta, documentation must always show the transparent manual commands in
addition to the convenience installer. The installer is not the trust root; it
only installs the repository key/source and invokes APT.

Target experience:

```bash
curl -fsSL https://get.extrittio.io/beta | sudo sh
sudoedit /etc/extrittio/extrittio.env
sudo systemctl enable --now extrittio
```

The script must:

- support only known Debian/Raspberry Pi OS releases and `arm64`, failing with
  an actionable error otherwise;
- require root explicitly and avoid piping an unaudited second download to a
  shell;
- install the signing key in `/usr/share/keyrings/`;
- use a `signed-by=` APT source entry, never deprecated global `apt-key`;
- display repository, channel, package version, and every action;
- be versioned, shellchecked, tested in CI, and downloadable for inspection;
- never overwrite `/etc/extrittio/extrittio.env`;
- never invent or transmit an administrator password;
- end by printing configuration, start, status, logs, backup, and uninstall
  commands.

Host the small static script behind `get.extrittio.io`, with source in this
repository. A redirect to an immutable versioned script is preferable to
serving mutable unreviewed content directly.

## 9. Upgrade, backup, and rollback policy

Beta upgrades are explicit:

```bash
sudo systemctl stop extrittio
sudo extrittio database --database-backend turso --deployment-profile edge \
  --data-dir /var/lib/extrittio backup /var/lib/extrittio/backups/pre-0.2.0-beta.2.db
sudo apt update
sudo apt install extrittio=0.2.0~beta.2
sudo systemctl start extrittio
curl --fail http://127.0.0.1:8080/health
```

Before public beta, provide a guarded helper or package workflow that performs
the stop, verified backup, upgrade, restart, and health check consistently. Do
not let `postinst` improvise database backups without enough space/error
handling or hide a failed migration behind a successful APT exit.

Rollback has two distinct cases:

- If the schema is backward compatible, install the previous exact package
  version retained in APT.
- If a migration is not backward compatible, stop the service, reinstall the
  previous package, and restore its verified pre-upgrade database backup.

Every release must declare its minimum readable schema and whether downgrade is
supported. A failed health check does not automatically restore a database;
that could discard writes accepted after startup.

## 10. Security and supply-chain requirements

- Pin third-party Actions by full commit SHA.
- Pin build container base images by digest.
- Build with locked Cargo and npm dependencies.
- Generate an SPDX SBOM for every package.
- Create GitHub provenance attestations using OIDC, with no long-lived signing
  key for the artifact attestation.
- Use a repository-scoped, environment-protected APT upload token; rotate it and
  never make it available to build/test jobs.
- Protect tags matching `v*` and restrict creation to release maintainers or a
  release automation identity.
- Enable immutable GitHub Releases before the first beta.
- Publish SHA-256 checksums, but treat signed repository metadata and provenance
  as the authenticity mechanisms.
- Run dependency policy, secret scanning, package lint, and license checks in
  the release gate.
- Never place bootstrap credentials, API keys, device certificates, database
  files, or firmware objects in artifacts or CI logs.

## 11. Implementation work packages

### WP1: Package contract and versioning

- Add tested SemVer-to-Debian version conversion.
- Make binary/package/tag versions agree.
- Add Debian documentation files and input validation.
- Define maintainer-script install/upgrade/remove behavior.
- Make insecure first start impossible.
- Update Edge deployment documentation.

**Exit:** a local command deterministically creates a lint-clean package with
the agreed contents and version.

### WP2: Reusable package CI

- Add `.github/workflows/debian-package.yml`.
- Add path selection for Debian/release inputs.
- Add package inspection, linkage, install, runtime, and state-preservation
  tests.
- Upload package, checksum, SBOM, and test summary as workflow artifacts.

**Exit:** pull requests prove packaging changes without external publication or
release secrets.

### WP3: Tagged beta release

- Add strict tag/source/changelog validation.
- Make tag builds run the full gate.
- Build once and pass artifacts between jobs.
- Add the protected `beta-release` environment and manual approval.
- Create attestations and a draft GitHub prerelease.

**Exit:** a test beta tag can create verified draft artifacts without uploading
to APT.

### WP4: Signed beta APT repository

- Provision provider namespace and `packages.extrittio.io` DNS.
- Configure signed `beta/main` repository metadata and retention.
- Store the least-privilege upload credential in the environment.
- Upload exact workflow artifacts and run clean-host index/install verification.
- Publish the GitHub prerelease only after APT verification.

**Exit:** `apt install extrittio=<exact-beta-version>` works on clean supported
hardware and the installed checksum matches the GitHub asset.

### WP5: Installer and operator runbook

- Add the auditable beta bootstrap script and CI tests.
- Host it at `get.extrittio.io/beta` through an immutable redirect.
- Document install, initial configuration, health, logs, backup, upgrade,
  rollback, pinning, removal, and bug-report collection.
- Add a release checklist template and support matrix.

**Exit:** a new Pi can go from supported OS to a healthy Extrittio service using
only the public runbook, without repository access.

### WP6: Real-hardware beta qualification

- Test Pi 4 and Pi 5 fresh install, reboot, upgrade, config preservation,
  database preservation, network interruption, disk-full behavior, and package
  downgrade/restore.
- Test both Wi-Fi-only mode and separately installed compatible OTBR/RCP mode.
- Record results and known issues against the release candidate.

**Exit:** the first beta release notes name the exact hardware and OS images
that passed qualification.

## 12. Promotion from beta to stable

Do not create a `stable` APT channel merely as an alias to beta. Enable stable
publication only after:

- at least two successful beta upgrade cycles on Pi 4 and Pi 5;
- backup and restore have been exercised on real user-shaped data;
- package install/upgrade/remove behavior is frozen and documented;
- the configuration format has a compatibility policy;
- schema compatibility and downgrade declarations are automated;
- signing-key rotation and repository recovery have been rehearsed;
- critical/high dependency and image findings are resolved or explicitly
  accepted;
- release monitoring and a security contact are operational;
- the first-run credential flow is safe;
- support windows and end-of-life behavior are published.

At stable launch:

- accept stable SemVer tags;
- publish stable packages to `stable/main`;
- mark GitHub releases as full releases rather than prereleases;
- keep beta opt-in and allow it to advance beyond stable;
- ensure stable users never receive beta solely because its version is newer;
- use an APT pin/channel configuration that requires explicit beta enrollment.

## 13. First beta acceptance checklist

- [ ] `v0.2.0-beta.1` is an allowed, protected release tag.
- [ ] `0.2.0~beta.1` is embedded and reported consistently.
- [ ] Full CI and Debian package tests pass for the tagged commit.
- [ ] Package was built once and the same digest was published everywhere.
- [ ] `.deb`, checksum, SBOM, provenance, and release notes are public.
- [ ] GitHub release immutability is enabled.
- [ ] Signed beta APT metadata verifies on a clean host.
- [ ] Exact-version installation works on Pi 4 and Pi 5.
- [ ] Edited config and seeded data survive an upgrade.
- [ ] Backup, failed upgrade, rollback, and restore procedures were rehearsed.
- [ ] Default credentials cannot expose a usable service.
- [ ] `get.extrittio.io/beta` is auditable and fails safely on unsupported hosts.
- [ ] Known limitations include OTBR packaging, supported OS versions, root
      service rationale, beta support expectations, and downgrade constraints.

