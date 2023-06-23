mod tvm;
mod transaction_emulator;
mod tvm_emulator;

use std::time::Duration;
use tonic::transport::Server;
use tracing_subscriber::EnvFilter;
use tracing_subscriber::fmt::format::FmtSpan;
use tonlibjson_sys::TvmEmulator;
use crate::tvm::transaction_emulator_service_server::TransactionEmulatorServiceServer;
use crate::tvm::tvm_emulator_service_server::TvmEmulatorServiceServer;
use crate::transaction_emulator::TransactionEmulatorService;
use crate::tvm_emulator::TvmEmulatorService;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    TvmEmulator::set_verbosity_level(0);

    tracing_subscriber::fmt()
        .with_env_filter(EnvFilter::from_default_env())
        .with_span_events(FmtSpan::CLOSE)
        .init();

    let reflection = tonic_reflection::server::Builder::configure()
        .register_encoded_file_descriptor_set(tonic_health::pb::FILE_DESCRIPTOR_SET)
        .register_encoded_file_descriptor_set(tvm::FILE_DESCRIPTOR_SET)
        .build()
        .unwrap();

    // TODO[akostylev0] env
    let addr = "0.0.0.0:50052".parse().unwrap();

    let (mut health_reporter, health_server) = tonic_health::server::health_reporter();
    health_reporter.set_serving::<TvmEmulatorServiceServer<TvmEmulatorService>>().await;
    health_reporter.set_serving::<TransactionEmulatorServiceServer<TransactionEmulatorService>>().await;

    let tvm_emulator_service = TvmEmulatorServiceServer::new(TvmEmulatorService::default());
    let transaction_emulator_service = TransactionEmulatorServiceServer::new(TransactionEmulatorService::default());

    Server::builder()
        .tcp_keepalive(Some(Duration::from_secs(120)))
        .http2_keepalive_interval(Some(Duration::from_secs(90)))
        .add_service(reflection)
        .add_service(health_server)
        .add_service(tvm_emulator_service)
        .add_service(transaction_emulator_service)
        .serve_with_shutdown(addr, async move { tokio::signal::ctrl_c().await.unwrap(); })
        .await?;

    Ok(())
}
