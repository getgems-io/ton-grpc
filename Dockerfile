FROM ghcr.io/akostylev0/tonlibjson-builder:sha-eba0f84fccae48d3da8ae71d22a7d344ac784004 AS builder

ARG FEATURES
ARG SCCACHE_GHA_ENABLED
ARG ACTIONS_CACHE_URL
ARG ACTIONS_RUNTIME_TOKEN

ENV SCCACHE_GHA_ENABLED=$SCCACHE_GHA_ENABLED
ENV ACTIONS_CACHE_URL=$ACTIONS_CACHE_URL
ENV ACTIONS_RUNTIME_TOKEN=$ACTIONS_RUNTIME_TOKEN

WORKDIR /app

COPY . .

RUN cargo fetch --locked
RUN cargo build -vv --release --target x86_64-unknown-linux-gnu --features "$FEATURES" && sccache --show-stats

RUN ldd /app/target/x86_64-unknown-linux-gnu/release/ton-grpc


FROM debian:bookworm-slim AS runner

RUN apt update && apt install --yes ca-certificates

COPY --from=builder /app/target/x86_64-unknown-linux-gnu/release/tonlibjson-jsonrpc /app/tonlibjson-jsonrpc
COPY --from=builder /app/target/x86_64-unknown-linux-gnu/release/ton-grpc /app/ton-grpc
COPY --from=builder /app/target/x86_64-unknown-linux-gnu/release/tvm-grpc /app/tvm-grpc

CMD ["/app/ton-grpc"]
