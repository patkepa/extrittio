# syntax=docker/dockerfile:1.7

# Stage 1: Cross-compile on native x86_64 for ARM64
FROM --platform=$BUILDPLATFORM docker.io/library/rust:1.90-bookworm@sha256:3914072ca0c3b8aad871db9169a651ccfce30cf58303e5d6f2db16d1d8a7e58f AS builder

ARG TARGETARCH
ARG CARGO_PROFILE=release

ENV CARGO_INCREMENTAL=0

RUN apt-get update && apt-get install -y \
    pkg-config \
    protobuf-compiler \
    && rm -rf /var/lib/apt/lists/*

# Install cross-compilation toolchain for ARM64
RUN if [ "$TARGETARCH" = "arm64" ]; then \
        dpkg --add-architecture arm64 && \
        apt-get update && \
        apt-get install -y gcc-aarch64-linux-gnu libpq-dev:arm64 libssl-dev:arm64 && \
        rm -rf /var/lib/apt/lists/* && \
        rustup target add aarch64-unknown-linux-gnu; \
    else \
        apt-get update && apt-get install -y libpq-dev libssl-dev && rm -rf /var/lib/apt/lists/*; \
    fi

# Configure cross-linker for ARM64
ENV CARGO_TARGET_AARCH64_UNKNOWN_LINUX_GNU_LINKER=aarch64-linux-gnu-gcc

WORKDIR /app

# Copy workspace manifests first for better layer caching
COPY Cargo.toml Cargo.lock ./
COPY crates/common/Cargo.toml crates/common/Cargo.toml
COPY crates/backend/Cargo.toml crates/backend/Cargo.toml
COPY crates/device-contract/Cargo.toml crates/device-contract/Cargo.toml
COPY crates/openthread-runtime/Cargo.toml crates/openthread-runtime/Cargo.toml
COPY crates/rule-engine/Cargo.toml crates/rule-engine/Cargo.toml
COPY apps/extrittio/Cargo.toml apps/extrittio/Cargo.toml
COPY clients/rust/linux/Cargo.toml clients/rust/linux/Cargo.toml
COPY clients/rust/macos/Cargo.toml clients/rust/macos/Cargo.toml
COPY clients/rust/rpi/Cargo.toml clients/rust/rpi/Cargo.toml
COPY clients/rust/runtime/Cargo.toml clients/rust/runtime/Cargo.toml
COPY clients/rust/sdk/Cargo.toml clients/rust/sdk/Cargo.toml
COPY clients/rust/simulator/Cargo.toml clients/rust/simulator/Cargo.toml
COPY tools/xtask/Cargo.toml tools/xtask/Cargo.toml

# Create dummy source files to pre-build dependencies
RUN mkdir -p crates/common/src && echo "fn main() {}" > crates/common/src/lib.rs && \
    mkdir -p crates/common/src/protos && \
    mkdir -p crates/backend/src && echo "fn main() {}" > crates/backend/src/main.rs && echo "" > crates/backend/src/lib.rs && \
    mkdir -p crates/device-contract/src && echo "" > crates/device-contract/src/lib.rs && \
    mkdir -p crates/openthread-runtime/src && echo "" > crates/openthread-runtime/src/lib.rs && \
    mkdir -p crates/rule-engine/src && echo "" > crates/rule-engine/src/lib.rs && \
    mkdir -p apps/extrittio/src && echo "fn main() {}" > apps/extrittio/src/main.rs && echo "" > apps/extrittio/src/lib.rs && \
    mkdir -p clients/rust/linux/src && echo "fn main() {}" > clients/rust/linux/src/main.rs && \
    mkdir -p clients/rust/macos/src && echo "fn main() {}" > clients/rust/macos/src/main.rs && \
    mkdir -p clients/rust/rpi/src && echo "fn main() {}" > clients/rust/rpi/src/main.rs && \
    mkdir -p clients/rust/runtime/src && echo "" > clients/rust/runtime/src/lib.rs && \
    mkdir -p clients/rust/sdk/src && echo "" > clients/rust/sdk/src/lib.rs && \
    mkdir -p clients/rust/simulator/src && echo "fn main() {}" > clients/rust/simulator/src/main.rs && \
    mkdir -p tools/xtask/src && echo "fn main() {}" > tools/xtask/src/main.rs

# Determine Rust target triple from TARGETARCH
RUN --mount=type=cache,target=/usr/local/cargo/registry \
    --mount=type=cache,target=/usr/local/cargo/git \
    --mount=type=cache,target=/app/target \
    if [ "$TARGETARCH" = "arm64" ]; then \
        PKG_CONFIG_ALLOW_CROSS=1 PKG_CONFIG_SYSROOT_DIR=/ \
        PKG_CONFIG_PATH=/usr/lib/aarch64-linux-gnu/pkgconfig \
        OPENSSL_LIB_DIR=/usr/lib/aarch64-linux-gnu OPENSSL_INCLUDE_DIR=/usr/include \
        cargo build --profile "$CARGO_PROFILE" --target aarch64-unknown-linux-gnu -p extrittio --no-default-features --features production; \
    else \
        cargo build --profile "$CARGO_PROFILE" -p extrittio --no-default-features --features production; \
    fi

# Copy actual source code
COPY crates/common/ crates/common/
COPY crates/backend/ crates/backend/
COPY crates/device-contract/ crates/device-contract/
COPY crates/openthread-runtime/ crates/openthread-runtime/
COPY crates/rule-engine/ crates/rule-engine/
COPY apps/extrittio/ apps/extrittio/

# Touch source files to invalidate the dummy build artifacts
RUN touch crates/common/src/lib.rs crates/common/build.rs \
    crates/backend/src/main.rs crates/backend/src/lib.rs crates/backend/build.rs \
    crates/device-contract/src/lib.rs crates/openthread-runtime/src/lib.rs \
    crates/rule-engine/src/lib.rs apps/extrittio/src/main.rs apps/extrittio/src/lib.rs

# Build the real binary
RUN --mount=type=cache,target=/usr/local/cargo/registry \
    --mount=type=cache,target=/usr/local/cargo/git \
    --mount=type=cache,target=/app/target \
    mkdir -p /app/bin && \
    if [ "$TARGETARCH" = "arm64" ]; then \
        PKG_CONFIG_ALLOW_CROSS=1 PKG_CONFIG_SYSROOT_DIR=/ \
        PKG_CONFIG_PATH=/usr/lib/aarch64-linux-gnu/pkgconfig \
        OPENSSL_LIB_DIR=/usr/lib/aarch64-linux-gnu OPENSSL_INCLUDE_DIR=/usr/include \
        cargo build --profile "$CARGO_PROFILE" --target aarch64-unknown-linux-gnu -p extrittio --no-default-features --features production && \
        cp "target/aarch64-unknown-linux-gnu/$CARGO_PROFILE/extrittio" /app/bin/extrittio; \
    else \
        cargo build --profile "$CARGO_PROFILE" -p extrittio --no-default-features --features production && \
        cp "target/$CARGO_PROFILE/extrittio" /app/bin/extrittio; \
    fi

# Stage 2: Build architecture-independent frontend assets on the native build platform.
FROM --platform=$BUILDPLATFORM docker.io/library/node:22-bookworm-slim@sha256:f32b81066cde10a75dbac96646099533316d94bac4150c55da1636e1f0ffdc46 AS frontend-builder

WORKDIR /frontend

COPY apps/frontend/package*.json ./
RUN --mount=type=cache,target=/root/.npm npm ci

COPY apps/frontend/ ./
RUN npm run build

# Stage 3: Minimal runtime image (runs on target arch)
FROM docker.io/library/debian:bookworm-slim@sha256:7b140f374b289a7c2befc338f42ebe6441b7ea838a042bbd5acbfca6ec875818

ARG EXTRITTIO_VERSION
ARG EXTRITTIO_COMMIT_SHA
ARG EXTRITTIO_BUILD_TIMESTAMP

RUN apt-get update && apt-get install -y libpq5 ca-certificates curl && rm -rf /var/lib/apt/lists/*

RUN useradd -r -s /bin/false extrittio

WORKDIR /app

COPY --from=builder /app/bin/extrittio .
COPY --from=frontend-builder /frontend/dist ./frontend

RUN mkdir -p /app/certs /app/data/firmware && chown -R extrittio:extrittio /app

USER extrittio

ENV PORT=8080
ENV CORS_ORIGIN=http://localhost:8080
ENV EXTRITTIO_UI_DIR=/app/frontend
ENV EXTRITTIO_VERSION=$EXTRITTIO_VERSION
ENV EXTRITTIO_COMMIT_SHA=$EXTRITTIO_COMMIT_SHA
ENV EXTRITTIO_BUILD_TIMESTAMP=$EXTRITTIO_BUILD_TIMESTAMP
ENV MALLOC_CONF=background_thread:true,dirty_decay_ms:5000,muzzy_decay_ms:5000
ENV RUST_LOG=extrittio=info,extrittio_backend=info

EXPOSE 8080

CMD ["./extrittio", "serve"]
