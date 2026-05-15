# Stage 1: Cross-compile on native x86_64 for ARM64
FROM --platform=$BUILDPLATFORM rust:1.90-bookworm AS builder

ARG TARGETARCH

RUN apt-get update && apt-get install -y \
    protobuf-compiler \
    && rm -rf /var/lib/apt/lists/*

# Install cross-compilation toolchain for ARM64
RUN if [ "$TARGETARCH" = "arm64" ]; then \
        dpkg --add-architecture arm64 && \
        apt-get update && \
        apt-get install -y gcc-aarch64-linux-gnu libpq-dev:arm64 && \
        rm -rf /var/lib/apt/lists/* && \
        rustup target add aarch64-unknown-linux-gnu; \
    else \
        apt-get update && apt-get install -y libpq-dev && rm -rf /var/lib/apt/lists/*; \
    fi

# Configure cross-linker for ARM64
ENV CARGO_TARGET_AARCH64_UNKNOWN_LINUX_GNU_LINKER=aarch64-linux-gnu-gcc

WORKDIR /app

# Copy workspace manifests first for better layer caching
COPY Cargo.toml Cargo.lock ./
COPY common/Cargo.toml common/Cargo.toml
COPY backend/Cargo.toml backend/Cargo.toml
COPY extrittio/Cargo.toml extrittio/Cargo.toml
COPY client/rust/linux/Cargo.toml client/rust/linux/Cargo.toml
COPY client/rust/macos/Cargo.toml client/rust/macos/Cargo.toml
COPY client/rust/rpi/Cargo.toml client/rust/rpi/Cargo.toml
COPY client/rust/sdk/Cargo.toml client/rust/sdk/Cargo.toml
COPY client/rust/simulator/Cargo.toml client/rust/simulator/Cargo.toml

# Create dummy source files to pre-build dependencies
RUN mkdir -p common/src && echo "fn main() {}" > common/src/lib.rs && \
    mkdir -p common/src/protos && \
    mkdir -p backend/src && echo "fn main() {}" > backend/src/main.rs && echo "" > backend/src/lib.rs && \
    mkdir -p extrittio/src && echo "fn main() {}" > extrittio/src/main.rs && echo "" > extrittio/src/lib.rs && \
    mkdir -p client/rust/linux/src && echo "fn main() {}" > client/rust/linux/src/main.rs && \
    mkdir -p client/rust/macos/src && echo "fn main() {}" > client/rust/macos/src/main.rs && \
    mkdir -p client/rust/rpi/src && echo "fn main() {}" > client/rust/rpi/src/main.rs && \
    mkdir -p client/rust/sdk/src && echo "" > client/rust/sdk/src/lib.rs && \
    mkdir -p client/rust/simulator/src && echo "fn main() {}" > client/rust/simulator/src/main.rs

# Determine Rust target triple from TARGETARCH
RUN if [ "$TARGETARCH" = "arm64" ]; then \
        cargo build --release --target aarch64-unknown-linux-gnu -p extrittio 2>/dev/null || true; \
    else \
        cargo build --release -p extrittio 2>/dev/null || true; \
    fi

# Copy actual source code
COPY common/ common/
COPY backend/ backend/
COPY extrittio/ extrittio/

# Touch source files to invalidate the dummy build artifacts
RUN touch common/src/lib.rs common/build.rs backend/src/main.rs backend/src/lib.rs extrittio/src/main.rs extrittio/src/lib.rs

# Build the real binary
RUN if [ "$TARGETARCH" = "arm64" ]; then \
        cargo build --release --target aarch64-unknown-linux-gnu -p extrittio && \
        cp target/aarch64-unknown-linux-gnu/release/extrittio target/release/extrittio; \
    else \
        cargo build --release -p extrittio; \
    fi

# Stage 2: Build the frontend assets served by the backend process
FROM node:22-bookworm-slim AS frontend-builder

WORKDIR /frontend

COPY frontend/package*.json ./
RUN npm ci

COPY frontend/ ./
RUN npm run build

# Stage 3: Minimal runtime image (runs on target arch)
FROM debian:bookworm-slim

ARG EXTRITTIO_VERSION
ARG EXTRITTIO_COMMIT_SHA
ARG EXTRITTIO_BUILD_TIMESTAMP

RUN apt-get update && apt-get install -y libpq5 ca-certificates curl && rm -rf /var/lib/apt/lists/*

RUN useradd -r -s /bin/false extrittio

WORKDIR /app

COPY --from=builder /app/target/release/extrittio .
COPY --from=frontend-builder /frontend/dist ./frontend

RUN mkdir -p /app/certs && chown -R extrittio:extrittio /app

USER extrittio

ENV PORT=8080
ENV CORS_ORIGIN=*
ENV EXTRITTIO_UI_DIR=/app/frontend
ENV EXTRITTIO_VERSION=$EXTRITTIO_VERSION
ENV EXTRITTIO_COMMIT_SHA=$EXTRITTIO_COMMIT_SHA
ENV EXTRITTIO_BUILD_TIMESTAMP=$EXTRITTIO_BUILD_TIMESTAMP
ENV RUST_LOG=extrittio=info,extrittio_backend=info

EXPOSE 8080

CMD ["./extrittio", "serve"]
