FROM ubuntu:noble

COPY target/x86_64-unknown-linux-gnu/release/ton-grpc /bin/ton-grpc

ENV OTLP=true

CMD ["/bin/ton-grpc"]
