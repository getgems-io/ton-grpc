mod tvm_emulator;

#[derive(Debug, Default)]
struct TransactionEmulatorService;

use std::time::Duration;
use tonic::transport::Server;
use crate::tvm_emulator::tvm_emulator_server::TvmEmulatorServer;
use crate::tvm_emulator::TvmEmulatorService;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt::init();
    tonlibjson_sys::TvmEmulator::set_verbosity_level(0);

    let reflection = tonic_reflection::server::Builder::configure()
        .register_encoded_file_descriptor_set(tonic_health::proto::GRPC_HEALTH_V1_FILE_DESCRIPTOR_SET)
        .register_encoded_file_descriptor_set(tvm_emulator::FILE_DESCRIPTOR_SET)
        .build()
        .unwrap();

    let addr = "0.0.0.0:50052".parse().unwrap();
    let svc = TvmEmulatorServer::new(TvmEmulatorService::default());

    let (mut health_reporter, health_service) = tonic_health::server::health_reporter();
    health_reporter.set_serving::<TvmEmulatorServer<TvmEmulatorService>>().await;

    Server::builder()
        .tcp_keepalive(Some(Duration::from_secs(1)))
        .http2_keepalive_interval(Some(Duration::from_secs(1)))
        .add_service(health_service)
        .add_service(reflection)
        .add_service(svc)
        .serve_with_shutdown(addr, async move { tokio::signal::ctrl_c().await.unwrap(); })
        .await?;

    Ok(())
}
