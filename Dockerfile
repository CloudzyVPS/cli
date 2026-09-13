# Stage 1: Build
FROM rust:1.91 AS builder

WORKDIR /build

# Cache dependencies separately from source
COPY Cargo.toml Cargo.lock* ./
RUN mkdir -p src && echo 'fn main() {}' > src/main.rs && \
    echo '' > src/lib.rs && \
    cargo build --release 2>/dev/null || true

# Build the real binary
COPY . .
RUN cargo build --release

# Stage 2: Runtime
FROM debian:bookworm-slim

RUN apt-get update && apt-get install -y --no-install-recommends \
    ca-certificates \
    && rm -rf /var/lib/apt/lists/*

COPY --from=builder /build/target/release/zy /usr/local/bin/zy

# Sign-ins persist in /config; mount a volume there, or pass CLOUDZY_TOKEN.
ENV CLOUDZY_URL=https://dash.cloudzy.com \
    CLOUDZY_CONFIG_DIR=/config
VOLUME ["/config"]

ENTRYPOINT ["zy"]
CMD ["--help"]
