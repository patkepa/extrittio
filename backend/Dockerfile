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
        apt-get install -y gcc-aarch64-linux-gnu libsqlite3-dev:arm64 && \
        rm -rf /var/lib/apt/lists/* && \
        rustup target add aarch64-unknown-linux-gnu; \
    else \
        apt-get update && apt-get install -y libsqlite3-dev && rm -rf /var/lib/apt/lists/*; \
    fi

# Configure cross-linker for ARM64
ENV CARGO_TARGET_AARCH64_UNKNOWN_LINUX_GNU_LINKER=aarch64-linux-gnu-gcc

WORKDIR /app

# Copy workspace manifests first for better layer caching
COPY Cargo.toml Cargo.lock ./
COPY proto/Cargo.toml proto/Cargo.toml
COPY backend/Cargo.toml backend/Cargo.toml
COPY client/linux/Cargo.toml client/linux/Cargo.toml

# Create dummy source files to pre-build dependencies
RUN mkdir -p proto/src && echo "fn main() {}" > proto/src/lib.rs && \
    mkdir -p proto/src/protos && \
    mkdir -p backend/src && echo "fn main() {}" > backend/src/main.rs && \
    mkdir -p client/linux/src && echo "fn main() {}" > client/linux/src/main.rs

# Determine Rust target triple from TARGETARCH
RUN if [ "$TARGETARCH" = "arm64" ]; then \
        cargo build --release --target aarch64-unknown-linux-gnu -p extrittio-backend 2>/dev/null || true; \
    else \
        cargo build --release -p extrittio-backend 2>/dev/null || true; \
    fi

# Copy actual source code
COPY proto/ proto/
COPY backend/ backend/

# Touch source files to invalidate the dummy build artifacts
RUN touch proto/src/lib.rs proto/build.rs backend/src/main.rs

# Build the real binary
RUN if [ "$TARGETARCH" = "arm64" ]; then \
        cargo build --release --target aarch64-unknown-linux-gnu -p extrittio-backend && \
        cp target/aarch64-unknown-linux-gnu/release/extrittio-backend target/release/extrittio-backend; \
    else \
        cargo build --release -p extrittio-backend; \
    fi

# Stage 2: Minimal runtime image (runs on target arch)
FROM debian:bookworm-slim

RUN apt-get update && apt-get install -y libsqlite3-0 ca-certificates && rm -rf /var/lib/apt/lists/*

RUN useradd -r -s /bin/false extrittio

WORKDIR /app

COPY --from=builder /app/target/release/extrittio-backend .

RUN mkdir -p /app/data && chown extrittio:extrittio /app/data

USER extrittio

ENV DATABASE_URL=/app/data/extrittio.db
ENV PORT=8080
ENV CORS_ORIGIN=*
ENV RUST_LOG=extrittio_backend=info

EXPOSE 8080

CMD ["./extrittio-backend"]
