# About

This repository contains Rust bindings for tonlibjson and services built on top of it.

### tonlibjson-sys
A Rust crate to wrap tonlibjson. Builds a statically linked lib from TON sources with cross language link time optimization (LTO).

### tonlibjson-client
A Rust client for The Open Network with p2c ewma balancer, automatic configuration reloading and retry budget.

See examples in `tonlibjson-client/examples`.

### ton-grpc

```bash
docker pull ghcr.io/akostylev0/tonlibjson:master
docker run --rm -p 50052:50052 ghcr.io/akostylev0/tonlibjson:master

```
