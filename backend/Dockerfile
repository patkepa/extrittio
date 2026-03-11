# Stage 1: Build the Rust backend
FROM rust:1.87-bookworm AS builder

RUN apt-get update && apt-get install -y protobuf-compiler libsqlite3-dev && rm -rf /var/lib/apt/lists/*

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

# Pre-build dependencies (this layer is cached unless Cargo.toml/Cargo.lock change)
RUN cargo build --release -p extrittio-backend 2>/dev/null || true

# Copy actual source code
COPY proto/ proto/
COPY backend/ backend/

# Touch source files to invalidate the dummy build artifacts
RUN touch proto/src/lib.rs proto/build.rs backend/src/main.rs

# Build the real binary
RUN cargo build --release -p extrittio-backend

# Stage 2: Minimal runtime image
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
