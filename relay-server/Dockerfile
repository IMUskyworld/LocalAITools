FROM rust:1.85-slim-bookworm AS builder

WORKDIR /src
ENV CARGO_TERM_COLOR=always \
    CARGO_NET_RETRY=10 \
    CARGO_BUILD_JOBS=1

RUN apt-get update \
    && apt-get install -y --no-install-recommends build-essential ca-certificates pkg-config \
    && rm -rf /var/lib/apt/lists/*

COPY shared-contract ./shared-contract
COPY relay-server ./relay-server
RUN cargo build --manifest-path relay-server/Cargo.toml --release

FROM debian:bookworm-slim AS runtime

RUN apt-get update \
    && apt-get install -y --no-install-recommends ca-certificates curl \
    && rm -rf /var/lib/apt/lists/* \
    && useradd --system --create-home --home-dir /var/lib/localmind-relay relay

COPY --from=builder /src/relay-server/target/release/localmind-relay /usr/local/bin/localmind-relay

USER relay
WORKDIR /var/lib/localmind-relay
ENV RELAY_BIND=0.0.0.0:8080 \
    RELAY_DB_PATH=/var/lib/localmind-relay/relay.db \
    RUST_LOG=info

EXPOSE 8080
VOLUME ["/var/lib/localmind-relay"]

HEALTHCHECK --interval=30s --timeout=5s --start-period=10s --retries=3 \
    CMD curl -fsS http://127.0.0.1:8080/health || exit 1

CMD ["localmind-relay"]