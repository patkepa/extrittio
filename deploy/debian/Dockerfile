# syntax=docker/dockerfile:1.7

# Build a Debian package containing the embedded-UI, Turso-backed Extrittio Edge
# executable only. OTBR is intentionally absent, but a system-installed agent
# is discovered and supervised at runtime when an RCP is connected.
FROM --platform=$TARGETPLATFORM docker.io/library/node:22-bookworm-slim@sha256:f32b81066cde10a75dbac96646099533316d94bac4150c55da1636e1f0ffdc46 AS node

FROM --platform=$TARGETPLATFORM docker.io/library/rust:1.90-bookworm@sha256:3914072ca0c3b8aad871db9169a651ccfce30cf58303e5d6f2db16d1d8a7e58f AS builder

ARG TARGETARCH
ARG PACKAGE_VERSION

RUN test "$TARGETARCH" = "arm64"

COPY --from=node /usr/local/bin/ /usr/local/bin/
COPY --from=node /usr/local/lib/node_modules/ /usr/local/lib/node_modules/

RUN apt-get update && apt-get install -y --no-install-recommends \
    build-essential \
    ca-certificates \
    dpkg-dev \
    libssl-dev \
    pkg-config \
    protobuf-compiler \
    && rm -rf /var/lib/apt/lists/*

ENV PATH=/usr/local/cargo/bin:/usr/local/bin:$PATH \
    CARGO_INCREMENTAL=0

WORKDIR /workspace
COPY . .

RUN --mount=type=cache,target=/root/.npm \
    --mount=type=cache,target=/root/.cargo/registry \
    --mount=type=cache,target=/root/.cargo/git \
    --mount=type=cache,target=/workspace/target \
    set -eu; \
    cargo xtask install edge-binary --release --root /opt/extrittio; \
    /opt/extrittio/bin/extrittio run --help >/dev/null; \
    smoke_dir="$(mktemp -d)"; \
    EXTRITTIO_THREAD_ENABLED=false /opt/extrittio/bin/extrittio -o json migrate --database-backend turso --deployment-profile edge --data-dir "$smoke_dir"; \
    EXTRITTIO_THREAD_ENABLED=false /opt/extrittio/bin/extrittio -o json database --database-backend turso --deployment-profile edge --data-dir "$smoke_dir" info; \
    ! ldd /opt/extrittio/bin/extrittio | grep -E 'libpq|postgres'

RUN set -eu; \
    test -n "$PACKAGE_VERSION"; \
    install -d debian; \
    printf '%s\n' \
      'Source: extrittio' \
      'Section: net' \
      'Priority: optional' \
      'Maintainer: Extrittio contributors <opensource@extrittio.dev>' \
      'Standards-Version: 4.6.2' \
      > debian/control; \
    package_root="/package-root"; \
    install -d "$package_root/DEBIAN" "$package_root/usr/bin" "$package_root/usr/lib/systemd/system" "$package_root/etc/extrittio"; \
    install -m 0755 /opt/extrittio/bin/extrittio "$package_root/usr/bin/extrittio"; \
    install -m 0644 deploy/debian/extrittio.service "$package_root/usr/lib/systemd/system/extrittio.service"; \
    install -m 0640 deploy/debian/extrittio.env.example "$package_root/etc/extrittio/extrittio.env"; \
    runtime_depends="$(dpkg-shlibdeps -O -e"$package_root/usr/bin/extrittio" | sed -n 's/^shlibs:Depends=//p')"; \
    test -n "$runtime_depends"; \
    printf '%s\n' \
      'Package: extrittio' \
      "Version: $PACKAGE_VERSION" \
      'Architecture: arm64' \
      'Maintainer: Extrittio contributors <opensource@extrittio.dev>' \
      "Depends: $runtime_depends, ca-certificates" \
      'Homepage: https://github.com/extrittio/extrittio' \
      'Description: Self-hosted IoT hub for single-node deployments' \
      ' Extrittio provides the embedded UI, API, messaging endpoint, and local' \
      ' Turso persistence in one executable. A locally installed OTBR agent is optional.' \
      > "$package_root/DEBIAN/control"; \
    printf '%s\n' '/etc/extrittio/extrittio.env' > "$package_root/DEBIAN/conffiles"; \
    printf '%s\n' \
      '#!/bin/sh' \
      'set -eu' \
      'install -d -o root -g root -m 0750 /var/lib/extrittio' \
      'if command -v systemctl >/dev/null 2>&1; then' \
      '  systemctl daemon-reload || true' \
      'fi' \
      > "$package_root/DEBIAN/postinst"; \
    chmod 0755 "$package_root/DEBIAN/postinst"; \
    install -d /artifact; \
    dpkg-deb --root-owner-group --build "$package_root" "/artifact/extrittio_${PACKAGE_VERSION}_arm64.deb"; \
    (cd /artifact && sha256sum "extrittio_${PACKAGE_VERSION}_arm64.deb" > "extrittio_${PACKAGE_VERSION}_arm64.deb.sha256")

FROM scratch AS artifact
COPY --from=builder /artifact/ /
