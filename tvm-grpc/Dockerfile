FROM gcr.io/distroless/cc-debian12

COPY ../target/x86_64-unknown-linux-gnu/release/tvm-grpc /bin/tvm-grpc

ENV OTLP=true

CMD ["/bin/tvm-grpc"]
