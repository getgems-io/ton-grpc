mod tvm_emulator;
mod transaction_emulator;
mod ton;
mod account;
mod helpers;
mod block;
mod message;

use std::time::Duration;
use tonic::transport::Server;
use tracing_subscriber::EnvFilter;
use tracing_subscriber::fmt::format::FmtSpan;
use tonlibjson_client::ton::TonClient;
use crate::account::AccountService;
use crate::block::BlockService;
use crate::ton::account_server::AccountServer;
use crate::ton::block_server::BlockServer;
use crate::ton::transaction_emulator_server::TransactionEmulatorServer;
use crate::ton::tvm_emulator_server::TvmEmulatorServer;
use crate::transaction_emulator::TransactionEmulatorService;
use crate::tvm_emulator::TvmEmulatorService;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(EnvFilter::from_default_env())
        .with_span_events(FmtSpan::CLOSE)
        .init();

    tonlibjson_sys::TvmEmulator::set_verbosity_level(0);

    let reflection = tonic_reflection::server::Builder::configure()
        .register_encoded_file_descriptor_set(tonic_health::pb::FILE_DESCRIPTOR_SET)
        .register_encoded_file_descriptor_set(ton::FILE_DESCRIPTOR_SET)
        .build()
        .unwrap();

    // TODO[akostylev0] env
    let addr = "0.0.0.0:50052".parse().unwrap();

    let mut client = TonClient::from_env().await?;

    client.ready().await?;

    tracing::info!("Ton Client is ready");

    let tvm_emulator_service = TvmEmulatorServer::new(TvmEmulatorService::default());
    let transaction_emulator_service = TransactionEmulatorServer::new(TransactionEmulatorService::default());
    let account_service = AccountServer::new(AccountService::new(client.clone()));
    let block_service = BlockServer::new(BlockService::new(client));

    let (mut health_reporter, health_service) = tonic_health::server::health_reporter();
    health_reporter.set_serving::<TvmEmulatorServer<TvmEmulatorService>>().await;

    Server::builder()
        .tcp_keepalive(Some(Duration::from_secs(120)))
        .http2_keepalive_interval(Some(Duration::from_secs(90)))
        .add_service(health_service)
        .add_service(reflection)
        .add_service(tvm_emulator_service)
        .add_service(transaction_emulator_service)
        .add_service(account_service)
        .add_service(block_service)
        .serve_with_shutdown(addr, async move { tokio::signal::ctrl_c().await.unwrap(); })
        .await?;

    Ok(())
}
