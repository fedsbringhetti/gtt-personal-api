# syntax=docker/dockerfile:1

# Build stages share the Rust toolchain and cargo-chef, which caches dependencies in their own
# layer: changing only our code does not recompile them.
FROM rust:1.95-slim-bookworm AS chef
RUN cargo install cargo-chef --locked
WORKDIR /app

FROM chef AS planner
COPY . .
RUN cargo chef prepare --recipe-path recipe.json

FROM chef AS builder
COPY --from=planner /app/recipe.json recipe.json
RUN cargo chef cook --release --recipe-path recipe.json
COPY . .
RUN cargo build --release --locked

FROM debian:bookworm-slim AS runtime
LABEL org.opencontainers.image.source="https://github.com/fedsbringhetti/gtt-personal-api" \
      org.opencontainers.image.description="Personal API for real-time GTT departures in Turin"
RUN apt-get update \
    && apt-get install -y --no-install-recommends ca-certificates tzdata \
    && rm -rf /var/lib/apt/lists/*
RUN useradd --system --uid 10001 --no-create-home --shell /usr/sbin/nologin app \
    && mkdir /data \
    && chown app /data
COPY --from=builder /app/target/release/gtt-personal-api /usr/local/bin/gtt-personal-api

USER app
ENV BIND_ADDR=0.0.0.0:8080 \
    DATA_DIR=/data \
    TZ=Europe/Rome \
    RUST_LOG=info
EXPOSE 8080
VOLUME /data

HEALTHCHECK --interval=30s --timeout=5s --start-period=10s --retries=3 \
    CMD ["gtt-personal-api", "healthcheck"]
ENTRYPOINT ["gtt-personal-api"]
