FROM ubuntu:noble

RUN apt-get update && apt-get install -y ca-certificates && rm -rf /var/lib/apt/lists/*

COPY target/x86_64-unknown-linux-gnu/release/ton-grpc /bin/ton-grpc

ENV OTLP=true

CMD ["/bin/ton-grpc"]
