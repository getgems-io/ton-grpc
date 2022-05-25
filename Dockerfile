FROM rust:1.61-bullseye

RUN apt update && apt install --yes --no-install-recommends cmake lsb-release software-properties-common
RUN wget https://apt.llvm.org/llvm.sh -O /tmp/llvm.sh && chmod +x /tmp/llvm.sh && /tmp/llvm.sh 14 all
RUN ln -sf $(which clang-14) /usr/bin/clang
RUN ln -sf $(which clang++-14) /usr/bin/clang++
RUN ln -sf /usr/bin/ld.lld-14 /usr/bin/ld.lld

COPY . .

RUN cargo fetch --locked

ENV RUSTFLAGS="-Clinker-plugin-lto -Clinker=clang -Clink-arg=-fuse-ld=lld"

RUN cargo build -vv --release

CMD ["./target/release/tonlibjson-jsonrpc"]
