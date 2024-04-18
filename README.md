# About

This repository contains Rust bindings for tonlibjson and services built on top of it.

## ton-grpc

```bash
docker pull ghcr.io/getgems-io/ton-grpc
docker run --rm -p 50052:50052 ghcr.io/getgems-io/ton-grpc
```

## ton-liteserver-client
### Installation
```toml
[dependencies]
ton-liteserver-client = { git = "https://github.com/getgems-io/ton-grpc.git" }
tokio = "1.37"
tower = "0.4"
```

### Usage
See `ton-liteserver-client/examples`