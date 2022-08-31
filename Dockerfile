FROM rust:1.63.0-bullseye AS builder

RUN apt update && apt install --yes --no-install-recommends cmake lsb-release software-properties-common

RUN wget https://apt.llvm.org/llvm.sh -O /tmp/llvm.sh && chmod +x /tmp/llvm.sh && /tmp/llvm.sh 14 all
RUN ln -sf $(which clang-14) /usr/bin/clang
RUN ln -sf $(which clang++-14) /usr/bin/clang++
RUN ln -sf /usr/bin/ld.lld-14 /usr/bin/ld.lld

RUN USER=root cargo new --bin app
WORKDIR /app

COPY ./Cargo.lock ./Cargo.lock
COPY ./Cargo.toml ./Cargo.toml

RUN USER=root cargo new --lib tonlibjson-rs
RUN USER=root cargo new --lib tonlibjson-tokio
RUN USER=root cargo new --bin tonlibjson-jsonrpc
RUN USER=root cargo new --bin tonlibjson-experiment

COPY ./tonlibjson-rs/Cargo.toml ./tonlibjson-rs/Cargo.toml
COPY ./tonlibjson-tokio/Cargo.toml ./tonlibjson-tokio/Cargo.toml
COPY ./tonlibjson-jsonrpc/Cargo.toml ./tonlibjson-jsonrpc/Cargo.toml
COPY ./tonlibjson-experiment/Cargo.toml ./tonlibjson-experiment/Cargo.toml

ADD .cargo .cargo

RUN cargo fetch --locked
RUN cargo build --release --target x86_64-unknown-linux-gnu

COPY . .

RUN cargo build -vv --release --target x86_64-unknown-linux-gnu
RUN cargo test -vv --release --target x86_64-unknown-linux-gnu

FROM debian:bullseye-slim AS runner

RUN apt update && apt install --yes ca-certificates

COPY --from=builder /app/target/x86_64-unknown-linux-gnu/release/tonlibjson-jsonrpc /app/tonlibjson-jsonrpc

CMD ["/app/tonlibjson-jsonrpc"]
