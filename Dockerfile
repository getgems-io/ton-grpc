FROM ghcr.io/akostylev0/tonlibjson-builder:sha-b4bf42cf16b23ac5b102b622f65dcf972611d90b AS builder

ARG FEATURES
ARG SCCACHE_GHA_ENABLED
ARG ACTIONS_CACHE_URL
ARG ACTIONS_RUNTIME_TOKEN

ENV SCCACHE_GHA_ENABLED=$SCCACHE_GHA_ENABLED
ENV ACTIONS_CACHE_URL=$ACTIONS_CACHE_URL
ENV ACTIONS_RUNTIME_TOKEN=$ACTIONS_RUNTIME_TOKEN

RUN USER=root cargo new --bin app
WORKDIR /app

COPY ./Cargo.lock ./Cargo.lock
COPY ./Cargo.toml ./Cargo.toml

RUN USER=root cargo new --lib tonlibjson-sys
RUN USER=root cargo new --lib tonlibjson-client
RUN USER=root cargo new --bin tonlibjson-jsonrpc
RUN USER=root cargo new --bin ton-grpc

COPY ./tonlibjson-sys/Cargo.toml ./tonlibjson-sys/Cargo.toml
COPY ./tonlibjson-client/Cargo.toml ./tonlibjson-client/Cargo.toml
COPY ./tonlibjson-jsonrpc/Cargo.toml ./tonlibjson-jsonrpc/Cargo.toml
COPY ./ton-grpc/Cargo.toml ./ton-grpc/Cargo.toml

ADD .cargo .cargo

RUN cargo fetch --locked
RUN cargo build --release --target x86_64-unknown-linux-gnu

COPY . .

RUN cargo build -vv --release --target x86_64-unknown-linux-gnu --features "$FEATURES"


FROM debian:bullseye-slim AS runner

RUN apt update && apt install --yes ca-certificates

COPY --from=builder /app/target/x86_64-unknown-linux-gnu/release/tonlibjson-jsonrpc /app/tonlibjson-jsonrpc
COPY --from=builder /app/target/x86_64-unknown-linux-gnu/release/ton-grpc /app/ton-grpc

CMD ["/app/tonlibjson-jsonrpc"]
