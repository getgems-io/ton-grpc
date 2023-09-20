FROM rust:1.72.0-bookworm

RUN apt-get update && apt-get install --yes --no-install-recommends cmake lsb-release software-properties-common unzip libsecp256k1-dev libsodium-dev

# TODO[akostylev0]
RUN wget https://apt.llvm.org/llvm.sh -O /tmp/llvm.sh && chmod +x /tmp/llvm.sh && /tmp/llvm.sh 16 all || true
RUN #apt-get update
RUN /tmp/llvm.sh 16 all

RUN ln -sf $(which clang-16) /usr/bin/clang
RUN ln -sf $(which clang++-16) /usr/bin/clang++
RUN ln -sf /usr/bin/ld.lld-16 /usr/bin/ld.lld

ENV CARGO_REGISTRIES_CRATES_IO_PROTOCOL=sparse

RUN wget https://github.com/mozilla/sccache/releases/download/v0.5.4/sccache-v0.5.4-x86_64-unknown-linux-musl.tar.gz
RUN tar xzf sccache-v0.5.4-x86_64-unknown-linux-musl.tar.gz \
    && mv sccache-v0.5.4-x86_64-unknown-linux-musl/sccache /usr/local/bin/sccache \
    && chmod +x /usr/local/bin/sccache

ENV RUSTC_WRAPPER=/usr/local/bin/sccache
ENV CMAKE_C_COMPILER_LAUNCHER=/usr/local/bin/sccache
ENV CMAKE_CXX_COMPILER_LAUNCHER=/usr/local/bin/sccache

RUN wget https://github.com/protocolbuffers/protobuf/releases/download/v22.2/protoc-22.2-linux-x86_64.zip
RUN unzip protoc-22.2-linux-x86_64.zip \
    && mv bin/protoc /usr/local/bin/protoc \
    && chmod +x /usr/local/bin/protoc \
    && mv include/* /usr/local/include/

RUN sccache --show-stats
RUN protoc --version

RUN cargo install clippy-sarif sarif-fmt