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
COPY client/linux/Cargo.toml client/linux/Cargo.toml
COPY client/macos/Cargo.toml client/macos/Cargo.toml
COPY client/rpi/Cargo.toml client/rpi/Cargo.toml
COPY client/sdk/Cargo.toml client/sdk/Cargo.toml

# Create dummy source files to pre-build dependencies
RUN mkdir -p common/src && echo "fn main() {}" > common/src/lib.rs && \
    mkdir -p common/src/protos && \
    mkdir -p backend/src && echo "fn main() {}" > backend/src/main.rs && \
    mkdir -p client/linux/src && echo "fn main() {}" > client/linux/src/main.rs && \
    mkdir -p client/macos/src && echo "fn main() {}" > client/macos/src/main.rs && \
    mkdir -p client/rpi/src && echo "fn main() {}" > client/rpi/src/main.rs && \
    mkdir -p client/sdk/src && echo "fn main() {}" > client/sdk/src/lib.rs

# Determine Rust target triple from TARGETARCH
RUN if [ "$TARGETARCH" = "arm64" ]; then \
        cargo build --release --target aarch64-unknown-linux-gnu -p extrittio-backend 2>/dev/null || true; \
    else \
        cargo build --release -p extrittio-backend 2>/dev/null || true; \
    fi

# Copy actual source code
COPY common/ common/
COPY backend/ backend/

# Touch source files to invalidate the dummy build artifacts
RUN touch common/src/lib.rs common/build.rs backend/src/main.rs

# Build the real binary
RUN if [ "$TARGETARCH" = "arm64" ]; then \
        cargo build --release --target aarch64-unknown-linux-gnu -p extrittio-backend && \
        cp target/aarch64-unknown-linux-gnu/release/extrittio-backend target/release/extrittio-backend; \
    else \
        cargo build --release -p extrittio-backend; \
    fi

# Stage 2: Minimal runtime image (runs on target arch)
FROM debian:bookworm-slim

RUN apt-get update && apt-get install -y libpq5 ca-certificates && rm -rf /var/lib/apt/lists/*

RUN useradd -r -s /bin/false extrittio

WORKDIR /app

COPY --from=builder /app/target/release/extrittio-backend .

USER extrittio

ENV PORT=8080
ENV CORS_ORIGIN=*
ENV RUST_LOG=extrittio_backend=info

EXPOSE 8080

CMD ["./extrittio-backend"]
