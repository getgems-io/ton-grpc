#[allow(clippy::enum_variant_names)] mod ton;
mod account;
mod helpers;
mod block;
mod message;

use std::time::Duration;
use metrics_exporter_prometheus::PrometheusBuilder;
use tonic::transport::Server;
use tonic::codec::CompressionEncoding::Gzip;
use tracing_subscriber::EnvFilter;
use tracing_subscriber::fmt::format::FmtSpan;
use tonlibjson_client::ton::TonClient;
use clap::Parser;
use crate::account::AccountService;
use crate::block::BlockService;
use crate::message::MessageService;
use crate::ton::account_service_server::AccountServiceServer;
use crate::ton::block_service_server::BlockServiceServer;
use crate::ton::message_service_server::MessageServiceServer;


#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
struct Args {
    #[clap(long, action)]
    enable_metrics: bool,
    #[clap(long, action, default_value_t = 10)]
    timeout: u64,
    #[clap(long, action, default_value_t = 10)]
    retry_budget_ttl: u64,
    #[clap(long, action, default_value_t = 1)]
    retry_min_rps: u64,
    #[clap(long, action, default_value_t = 0.1)]
    retry_withdraw_percent: f32,
    #[clap(long, action, default_value_t = 128)]
    retry_first_delay_millis: u32,
    #[clap(long, action, default_value_t = 4096)]
    retry_max_delay_millis: u32,
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let args = Args::parse();

    tracing_subscriber::fmt()
        .with_env_filter(EnvFilter::from_default_env())
        .with_span_events(FmtSpan::CLOSE)
        .init();

    if args.enable_metrics {
        PrometheusBuilder::new()
            .install()
            .expect("failed to install Prometheus recorder");
    }

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

    let account_service = AccountServiceServer::new(AccountService::new(client.clone()))
        .accept_compressed(Gzip)
        .send_compressed(Gzip);
    let block_service = BlockServiceServer::new(BlockService::new(client.clone()))
        .accept_compressed(Gzip)
        .send_compressed(Gzip);
    let message_service = MessageServiceServer::new(MessageService::new(client))
        .accept_compressed(Gzip)
        .send_compressed(Gzip);

    let (mut health_reporter, health_server) = tonic_health::server::health_reporter();
    health_reporter.set_serving::<AccountServiceServer<AccountService>>().await;
    health_reporter.set_serving::<BlockServiceServer<BlockService>>().await;
    health_reporter.set_serving::<MessageServiceServer<MessageService>>().await;

    Server::builder()
        .tcp_keepalive(Some(Duration::from_secs(300)))
        .http2_keepalive_interval(Some(Duration::from_secs(120)))
        .http2_keepalive_timeout(Some(Duration::from_secs(20)))
        .timeout(Duration::from_secs(args.timeout))
        .add_service(reflection)
        .add_service(health_server)
        .add_service(account_service)
        .add_service(block_service)
        .add_service(message_service)
        .serve_with_shutdown(addr, async move { tokio::signal::ctrl_c().await.unwrap(); })
        .await?;

    Ok(())
}
