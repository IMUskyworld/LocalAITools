ARG DEBIAN_MIRROR=mirrors.aliyun.com

FROM rust:1.85-slim-bookworm AS builder
ARG DEBIAN_MIRROR

WORKDIR /src
ENV CARGO_TERM_COLOR=always \
    CARGO_NET_RETRY=10 \
    CARGO_BUILD_JOBS=1

RUN set -eux; \
    if [ -f /etc/apt/sources.list.d/debian.sources ]; then \
        sed -i "s|deb.debian.org|${DEBIAN_MIRROR}|g" /etc/apt/sources.list.d/debian.sources; \
    fi; \
    if [ -f /etc/apt/sources.list ]; then \
        sed -i "s|deb.debian.org|${DEBIAN_MIRROR}|g" /etc/apt/sources.list; \
    fi; \
    apt-get -o Acquire::Retries=5 -o Acquire::http::Timeout=30 -o Acquire::https::Timeout=30 update; \
    apt-get install -y --no-install-recommends build-essential ca-certificates pkg-config; \
    rm -rf /var/lib/apt/lists/*

COPY shared-contract ./shared-contract
COPY relay-server ./relay-server
RUN cargo build --manifest-path relay-server/Cargo.toml --release

FROM debian:bookworm-slim AS runtime
ARG DEBIAN_MIRROR

RUN set -eux; \
    if [ -f /etc/apt/sources.list.d/debian.sources ]; then \
        sed -i "s|deb.debian.org|${DEBIAN_MIRROR}|g" /etc/apt/sources.list.d/debian.sources; \
    fi; \
    if [ -f /etc/apt/sources.list ]; then \
        sed -i "s|deb.debian.org|${DEBIAN_MIRROR}|g" /etc/apt/sources.list; \
    fi; \
    apt-get -o Acquire::Retries=5 -o Acquire::http::Timeout=30 -o Acquire::https::Timeout=30 update; \
    apt-get install -y --no-install-recommends ca-certificates curl; \
    rm -rf /var/lib/apt/lists/*; \
    useradd --system --create-home --home-dir /var/lib/localmind-relay relay

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
