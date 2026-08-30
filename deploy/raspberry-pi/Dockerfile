# syntax=docker/dockerfile:1.7

# This image deliberately runs as the target platform. On an Apple Silicon Mac,
# linux/arm64 is native inside Docker Desktop; on another host Buildx uses its
# configured arm64 builder. Do not build Extrittio Edge on macOS itself:
# both the Rust executable and otbr-agent must be Linux binaries.
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
    cmake \
    git \
    libavahi-client-dev \
    libavahi-common-dev \
    libboost-dev \
    libdbus-1-dev \
    libjsoncpp-dev \
    libprotobuf-dev \
    libreadline-dev \
    libssl-dev \
    ninja-build \
    pkg-config \
    protobuf-compiler \
    python3 \
    && rm -rf /var/lib/apt/lists/*

ENV PATH=/usr/local/cargo/bin:/usr/local/bin:$PATH \
    CARGO_INCREMENTAL=0

WORKDIR /workspace
COPY . .

# The frontend's framework dependency is public on npm.
# Keep Linux CMake metadata separate from a native macOS build when the target
# cache is reused by Docker Desktop.
RUN --mount=type=cache,target=/root/.npm \
    --mount=type=cache,target=/root/.cargo/registry \
    --mount=type=cache,target=/root/.cargo/git \
    --mount=type=cache,target=/workspace/target \
    set -eu; \
    cargo xtask install edge --release --root /opt/extrittio \
        --otbr-build-dir target/openthread/build-linux-arm64; \
    /opt/extrittio/bin/extrittio run --help >/dev/null; \
    smoke_dir="$(mktemp -d)"; \
    /opt/extrittio/bin/extrittio -o json migrate --database-backend turso --deployment-profile edge --data-dir "$smoke_dir"; \
    /opt/extrittio/bin/extrittio -o json database --database-backend turso --deployment-profile edge --data-dir "$smoke_dir" info; \
    ! ldd /opt/extrittio/bin/extrittio | grep -E 'libpq|postgres'; \
    ! ldd /opt/extrittio/libexec/extrittio/otbr-agent | grep 'not found'

RUN test -n "$PACKAGE_VERSION" && \
    package_dir="/artifact/extrittio-edge-linux-arm64-$PACKAGE_VERSION" && \
    install -d "$package_dir" && \
    cp -a /opt/extrittio/bin /opt/extrittio/libexec "$package_dir/" && \
    printf '{"package":"extrittio-edge-linux-arm64","version":"%s","base":"debian-bookworm"}\n' "$PACKAGE_VERSION" > "$package_dir/manifest.json" && \
    (cd "$package_dir" && sha256sum bin/extrittio libexec/extrittio/otbr-agent > SHA256SUMS)

# Buildx exports this stage to the Mac; it is an appliance release directory,
# not a runtime container image.
FROM scratch AS artifact
COPY --from=builder /artifact/ /
