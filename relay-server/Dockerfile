# The official Rust slim image already contains gcc, libc headers and CA certs.
# Avoid apt during the build so deployment does not depend on Debian mirrors.
FROM rust:1.85-slim-bookworm AS builder

WORKDIR /src
ENV CARGO_TERM_COLOR=always \
    CARGO_NET_RETRY=10 \
    CARGO_BUILD_JOBS=1

COPY shared-contract ./shared-contract
COPY relay-server ./relay-server
RUN cargo build --manifest-path relay-server/Cargo.toml --release

FROM debian:bookworm-slim AS runtime

RUN useradd --system --create-home --home-dir /var/lib/localmind-relay relay

COPY --from=builder /src/relay-server/target/release/localmind-relay /usr/local/bin/localmind-relay

USER relay
WORKDIR /var/lib/localmind-relay
ENV RELAY_BIND=0.0.0.0:8080 \
    RELAY_DB_PATH=/var/lib/localmind-relay/relay.db \
    RUST_LOG=info

EXPOSE 8080
VOLUME ["/var/lib/localmind-relay"]

CMD ["localmind-relay"]
