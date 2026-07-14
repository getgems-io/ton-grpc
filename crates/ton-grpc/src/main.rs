use clap::{Args, Parser, ValueEnum};
use humantime::parse_duration;
use metrics_exporter_prometheus::PrometheusBuilder;
use std::net::SocketAddr;
use std::path::PathBuf;
use std::time::Duration;
use ton_client::{ConfigSource, PoolTransport, TonClientBuilder, TonService};
use ton_config::{TonConfig, default_ton_config_url};
use ton_grpc::AccountService;
use ton_grpc::BlockService;
use ton_grpc::MessageService;
use ton_grpc::account_service_server::AccountServiceServer;
use ton_grpc::block_service_server::BlockServiceServer;
use ton_grpc::message_service_server::MessageServiceServer;
use ton_liteserver_client::MakeLiteServerAdapter;
use tonic::codec::CompressionEncoding::Gzip;
use tonic::transport::Server;
use tonlibjson_client::MakeTonlibjsonAdapter;
use tower::Service;
use tracing_subscriber::EnvFilter;
use tracing_subscriber::fmt::format::FmtSpan;
use url::Url;

#[derive(Copy, Clone, Debug, PartialEq, Eq, ValueEnum)]
enum ClientImpl {
    Tonlibjson,
    AdnlTcp,
}

#[derive(Args, Debug)]
#[group(multiple = false)]
struct TonConfigArgs {
    #[clap(long, value_parser = Url::parse)]
    ton_config_url: Option<Url>,
    #[clap(long)]
    ton_config_path: Option<PathBuf>,
}

impl From<TonConfigArgs> for ConfigSource {
    fn from(value: TonConfigArgs) -> Self {
        if let Some(path) = value.ton_config_path {
            return Self::File { path };
        }

        Self::Url {
            url: value.ton_config_url.unwrap_or(default_ton_config_url()),
            interval: Duration::from_secs(60),
        }
    }
}

#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
struct AppArgs {
    #[clap(long, default_value = "0.0.0.0:50052")]
    listen: SocketAddr,
    #[clap(long, value_parser = parse_duration, default_value = "30s")]
    timeout: Duration,
    #[clap(long, value_parser = parse_duration, default_value = "300s")]
    tcp_keepalive: Duration,
    #[clap(long, value_parser = parse_duration, default_value = "120s")]
    http2_keepalive_interval: Duration,
    #[clap(long, value_parser = parse_duration, default_value = "20s")]
    http2_keepalive_timeout: Duration,
    #[clap(long, default_value = "65535")]
    initial_connection_window_size: u32,
    #[clap(long, default_value = "65535")]
    initial_stream_window_size: u32,

    #[clap(long)]
    enable_metrics: bool,
    #[clap(long, default_value = "0.0.0.0:9000")]
    metrics_listen: SocketAddr,

    #[clap(long, value_enum, default_value_t = ClientImpl::Tonlibjson)]
    client: ClientImpl,

    #[clap(flatten)]
    ton_config_args: TonConfigArgs,
    #[clap(long, value_parser = parse_duration, default_value = "10s")]
    ton_timeout: Duration,
    #[clap(long, value_parser = parse_duration, default_value = "10s")]
    retry_budget_ttl: Duration,
    #[clap(long, default_value_t = 1)]
    retry_min_rps: u32,
    #[clap(long, default_value_t = 0.1)]
    retry_withdraw_percent: f32,
    #[clap(long, value_parser = parse_duration, default_value = "128ms")]
    retry_first_delay: Duration,
    #[clap(long, value_parser = parse_duration, default_value = "4096ms")]
    retry_max_delay: Duration,

    #[clap(long, value_parser = parse_duration, default_value = "70ms")]
    ewma_default_rtt: Duration,
    #[clap(long, value_parser = parse_duration, default_value = "1ms")]
    ewma_decay: Duration,
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let args = AppArgs::parse();

    tracing_subscriber::fmt()
        .with_env_filter(EnvFilter::from_default_env())
        .with_span_events(FmtSpan::CLOSE)
        .init();

    if args.enable_metrics {
        PrometheusBuilder::new()
            .with_http_listener(args.metrics_listen)
            .install()
            .expect("failed to install Prometheus recorder");

        tracing::info!("Listening metrics on {:?}", &args.metrics_listen);
    }

    match args.client {
        ClientImpl::Tonlibjson => serve(args, MakeTonlibjsonAdapter).await,
        ClientImpl::AdnlTcp => serve(args, MakeLiteServerAdapter).await,
    }
}

async fn serve<F>(args: AppArgs, factory: F) -> anyhow::Result<()>
where
    F: Service<TonConfig, Response: TonService, Error: Send + Sync, Future: Send + Unpin>
        + Clone
        + Send
        + 'static,
    anyhow::Error: From<F::Error>,
{
    let config_source = ConfigSource::from(args.ton_config_args);
    match &config_source {
        ConfigSource::File { path } => tracing::info!("TON Config path: {}", path.display()),
        ConfigSource::Url { url, .. } => tracing::info!("TON Config URL: {}", url),
        ConfigSource::Config { .. } => tracing::info!("TON Config: inline"),
    }
    tracing::info!("Client implementation: {:?}", &args.client);

    let mut client = TonClientBuilder::<F>::with_factory_and_source(factory, config_source)
        .set_timeout(args.ton_timeout)
        .set_retry_budget_ttl(args.retry_budget_ttl)
        .set_retry_min_per_sec(args.retry_min_rps)
        .set_retry_percent(args.retry_withdraw_percent)
        .set_retry_first_delay(args.retry_first_delay)
        .set_retry_max_delay(args.retry_max_delay)
        .set_ewma_default_rtt(args.ewma_default_rtt)
        .set_ewma_decay(args.ewma_decay)
        .build()?;

    client.wait_ready().await?;
    tracing::info!("Ton Client is ready");

    let reflection = tonic_reflection::server::Builder::configure()
        .register_encoded_file_descriptor_set(tonic_health::pb::FILE_DESCRIPTOR_SET)
        .register_encoded_file_descriptor_set(ton_grpc::ton::FILE_DESCRIPTOR_SET)
        .build_v1()?;

    let account_service = AccountServiceServer::new(AccountService::new(client.clone()))
        .accept_compressed(Gzip)
        .send_compressed(Gzip);
    let block_service = BlockServiceServer::new(BlockService::new(client.clone()))
        .accept_compressed(Gzip)
        .send_compressed(Gzip);
    let message_service = MessageServiceServer::new(MessageService::new(client))
        .accept_compressed(Gzip)
        .send_compressed(Gzip);

    let (health_reporter, health_server) = tonic_health::server::health_reporter();
    health_reporter
        .set_serving::<AccountServiceServer<AccountService<PoolTransport<F>>>>()
        .await;
    health_reporter
        .set_serving::<BlockServiceServer<BlockService<PoolTransport<F>>>>()
        .await;
    health_reporter
        .set_serving::<MessageServiceServer<MessageService<PoolTransport<F>>>>()
        .await;

    tracing::info!("Listening on {:?}", &args.listen);

    Server::builder()
        .timeout(args.timeout)
        .tcp_keepalive(args.tcp_keepalive.into())
        .http2_keepalive_interval(args.http2_keepalive_interval.into())
        .http2_keepalive_timeout(args.http2_keepalive_timeout.into())
        .initial_connection_window_size(args.initial_connection_window_size)
        .initial_stream_window_size(args.initial_stream_window_size)
        .add_service(reflection)
        .add_service(health_server)
        .add_service(account_service)
        .add_service(block_service)
        .add_service(message_service)
        .serve_with_shutdown(args.listen, async move {
            tokio::signal::ctrl_c().await.unwrap();
        })
        .await?;

    Ok(())
}
