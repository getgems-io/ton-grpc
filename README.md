# About

This repository contains Rust bindings for tonlibjson and services built on top of it.

### tonlibjson-sys
A Rust crate to wrap tonlibjson. Builds a statically linked lib from TON sources with cross language link time optimization (LTO).

### tonlibjson-client
A Rust client for The Open Network with p2c ewma balancer, automatic configuration reloading and retry budget.

See examples in `tonlibjson-client/examples`.

### tonlibjson-jsonrpc
Developed as a drop-in replacement for Toncenter for Getgems needs. Supports only jsonrpc interface and only a subset of methods from original Toncenter.


```bash
docker pull ghcr.io/akostylev0/tonlibjson:master
docker run --rm -p 3030:3030 ghcr.io/akostylev0/tonlibjson:master

curl --request POST 'http://localhost:3030/' \
--header 'Content-Type: application/json' \
--data-raw '{
    "jsonrpc": "2.0",
    "id": 1,
    "method": "getMasterchainInfo"
}'
```