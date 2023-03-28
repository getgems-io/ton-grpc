FROM rust:1.68.1-bullseye

RUN apt update && apt install --yes --no-install-recommends cmake lsb-release software-properties-common unzip

RUN wget https://apt.llvm.org/llvm.sh -O /tmp/llvm.sh && chmod +x /tmp/llvm.sh && /tmp/llvm.sh 15 all
RUN ln -sf $(which clang-15) /usr/bin/clang
RUN ln -sf $(which clang++-15) /usr/bin/clang++
RUN ln -sf /usr/bin/ld.lld-15 /usr/bin/ld.lld


RUN wget https://github.com/mozilla/sccache/releases/download/v0.4.1/sccache-v0.4.1-x86_64-unknown-linux-musl.tar.gz
RUN tar xzf sccache-v0.4.1-x86_64-unknown-linux-musl.tar.gz \
    && mv sccache-v0.4.1-x86_64-unknown-linux-musl/sccache /usr/local/bin/sccache \
    && chmod +x /usr/local/bin/sccache

ENV RUSTC_WRAPPER=/usr/local/bin/sccache
ENV CMAKE_CC_COMPILER_LAUNCHER=/usr/local/bin/sccache
ENV CMAKE_CXX_COMPILER_LAUNCHER=/usr/local/bin/sccache

RUN sccache --show-stats
