# -- Stage 1: Build --
FROM rust:1.88-slim-bookworm AS builder

RUN apt-get update && apt-get install -y --no-install-recommends \
    build-essential \
    cmake \
    pkg-config \
    libssl-dev \
    libcurl4-openssl-dev \
    libsasl2-dev \
    curl \
    && rm -rf /var/lib/apt/lists/*

WORKDIR /build

# Cache dependencies
COPY Cargo.toml Cargo.lock ./
RUN mkdir -p src && echo 'fn main() {}' > src/main.rs && \
    cargo build --release --bin ods-securemail 2>&1 | tail -5; \
    rm -rf src

# Build the real application
COPY migrations ./migrations
COPY src ./src
RUN touch src/main.rs && cargo build --release --bin ods-securemail

# -- Stage 2: Runtime --
FROM debian:bookworm-slim AS runtime

RUN apt-get update && apt-get install -y --no-install-recommends \
    ca-certificates \
    libssl3 \
    libsasl2-2 \
    curl \
    && rm -rf /var/lib/apt/lists/*

RUN useradd --uid 1000 --no-create-home --shell /sbin/nologin securemail

WORKDIR /app

COPY --from=builder /build/target/release/ods-securemail /app/ods-securemail

RUN chown -R securemail:securemail /app

USER securemail

EXPOSE 8086

HEALTHCHECK --interval=30s --timeout=5s --start-period=15s --retries=3 \
    CMD curl -sf http://localhost:8086/health || exit 1

CMD ["/app/ods-securemail"]
