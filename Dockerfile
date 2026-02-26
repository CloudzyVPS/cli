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

EXPOSE 5000

ENV API_BASE_URL=https://api.cloudzy.com/developers \
    PUBLIC_BASE_URL=http://localhost:5000

ENTRYPOINT ["zy"]
CMD ["serve", "--host", "0.0.0.0", "--port", "5000"]
