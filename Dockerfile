FROM rust:1.67.0-bullseye AS builder

ARG FEATURES

RUN apt update && apt install --yes --no-install-recommends cmake lsb-release software-properties-common

RUN wget https://apt.llvm.org/llvm.sh -O /tmp/llvm.sh && chmod +x /tmp/llvm.sh && /tmp/llvm.sh 15 all
RUN ln -sf $(which clang-15) /usr/bin/clang
RUN ln -sf $(which clang++-15) /usr/bin/clang++
RUN ln -sf /usr/bin/ld.lld-15 /usr/bin/ld.lld

RUN USER=root cargo new --bin app
WORKDIR /app

COPY ./Cargo.lock ./Cargo.lock
COPY ./Cargo.toml ./Cargo.toml

RUN USER=root cargo new --lib tonlibjson-sys
RUN USER=root cargo new --lib tonlibjson-client
RUN USER=root cargo new --bin tonlibjson-jsonrpc

COPY ./tonlibjson-sys/Cargo.toml ./tonlibjson-sys/Cargo.toml
COPY ./tonlibjson-client/Cargo.toml ./tonlibjson-client/Cargo.toml
COPY ./tonlibjson-jsonrpc/Cargo.toml ./tonlibjson-jsonrpc/Cargo.toml

ADD .cargo .cargo

RUN cargo fetch --locked
RUN cargo build --release --target x86_64-unknown-linux-gnu

COPY . .

RUN cargo build -vv --release --target x86_64-unknown-linux-gnu --features "$FEATURES"

FROM debian:bullseye-slim AS runner

RUN apt update && apt install --yes ca-certificates

COPY --from=builder /app/target/x86_64-unknown-linux-gnu/release/tonlibjson-jsonrpc /app/tonlibjson-jsonrpc

CMD ["/app/tonlibjson-jsonrpc"]
