mod ton;
mod account;
mod helpers;
mod block;
mod message;

use std::env;
use std::time::Duration;
use tonic::transport::Server;
use tower_http::trace::{DefaultMakeSpan, TraceLayer};
use tracing_subscriber::EnvFilter;
use tracing_subscriber::fmt::format::FmtSpan;
use tracing_subscriber::layer::SubscriberExt;
use tracing_subscriber::util::SubscriberInitExt;
use tonlibjson_client::ton::TonClient;
use crate::account::AccountService;
use crate::block::BlockService;
use crate::message::MessageService;
use crate::ton::account_service_server::AccountServiceServer;
use crate::ton::block_service_server::BlockServiceServer;
use crate::ton::message_service_server::MessageServiceServer;


#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let otlp = env::var("OTLP").unwrap_or_default()
        .parse::<bool>().unwrap_or(false);

    if otlp { init_tracing_otlp()? } else { init_tracing()? }

    tracing::info!(otlp = ?otlp, "tracing initialized");

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

    let account_service = AccountServiceServer::new(AccountService::new(client.clone()));
    let block_service = BlockServiceServer::new(BlockService::new(client.clone()));
    let message_service = MessageServiceServer::new(MessageService::new(client));

    Server::builder()
        .layer(TraceLayer::new_for_grpc()
            .make_span_with(DefaultMakeSpan::new()
                .level(tracing::Level::INFO)
                .include_headers(true)))
        .tcp_keepalive(Some(Duration::from_secs(120)))
        .http2_keepalive_interval(Some(Duration::from_secs(90)))
        .add_service(reflection)
        .add_service(account_service)
        .add_service(block_service)
        .add_service(message_service)
        .serve_with_shutdown(addr, async move { tokio::signal::ctrl_c().await.unwrap(); })
        .await?;

    Ok(())
}

fn init_tracing() -> anyhow::Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(EnvFilter::from_default_env())
        .with_span_events(FmtSpan::CLOSE)
        .init();

    Ok(())
}

fn init_tracing_otlp() -> anyhow::Result<()> {
    use opentelemetry_otlp::WithExportConfig;

    let fmt_layer = tracing_subscriber::fmt::layer()
        .with_span_events(FmtSpan::CLOSE);

    let tracer = opentelemetry_otlp::new_pipeline()
        .tracing()
        .with_exporter(opentelemetry_otlp::new_exporter().tonic().with_env())
        .install_batch(opentelemetry::runtime::Tokio)?;

    let telemetry_layer = tracing_opentelemetry::layer()
        .with_tracer(tracer);

    tracing_subscriber::registry()
        .with(telemetry_layer)
        .with(EnvFilter::from_default_env())
        .with(fmt_layer)
        .init();

    Ok(())
}
