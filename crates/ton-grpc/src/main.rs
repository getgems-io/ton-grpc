use clap::Parser;
use clap::ValueEnum;
use metrics_exporter_prometheus::PrometheusBuilder;
use std::net::SocketAddr;
use std::path::PathBuf;
use std::time::Duration;
use ton_client::ConfigSource;
use ton_client::PoolTransport;
use ton_client::TonClientBuilder;
use ton_config::default_ton_config_url;
use ton_grpc::AccountService;
use ton_grpc::BlockService;
use ton_grpc::ComparingAdapter;
use ton_grpc::MakeComparingAdapter;
use ton_grpc::MessageService;
use ton_grpc::account_service_server::AccountServiceServer;
use ton_grpc::block_service_server::BlockServiceServer;
use ton_grpc::message_service_server::MessageServiceServer;
use ton_liteserver_client::{LiteServerAdapter, MakeLiteServerAdapter};
use tonic::codec::CompressionEncoding::Gzip;
use tonic::transport::Server;
use tonlibjson_client::{MakeTonlibjsonAdapter, TonlibjsonAdapter};
use tower::ServiceExt;
use tower::util::{Either, MapResponse};
use tracing_subscriber::EnvFilter;
use tracing_subscriber::fmt::format::FmtSpan;
use url::Url;

type ComparingTonlibAdapter = ComparingAdapter<TonlibjsonAdapter, LiteServerAdapter>;
type ComparingAdnlAdapter = ComparingAdapter<LiteServerAdapter, TonlibjsonAdapter>;

type SingleAdapter = Either<TonlibjsonAdapter, LiteServerAdapter>;
type ComparingPair = Either<ComparingTonlibAdapter, ComparingAdnlAdapter>;

type Adapter = Either<SingleAdapter, ComparingPair>;
type Client = PoolTransport<Adapter>;

type MakeTonlibjsonSingle = MapResponse<MakeTonlibjsonAdapter, fn(TonlibjsonAdapter) -> Adapter>;
type MakeLiteServerSingle = MapResponse<MakeLiteServerAdapter, fn(LiteServerAdapter) -> Adapter>;
type MakeComparingTonlib = MapResponse<
    MakeComparingAdapter<MakeTonlibjsonAdapter, MakeLiteServerAdapter>,
    fn(ComparingTonlibAdapter) -> Adapter,
>;
type MakeComparingAdnl = MapResponse<
    MakeComparingAdapter<MakeLiteServerAdapter, MakeTonlibjsonAdapter>,
    fn(ComparingAdnlAdapter) -> Adapter,
>;

type SingleFactory = Either<MakeTonlibjsonSingle, MakeLiteServerSingle>;
type ComparingFactory = Either<MakeComparingTonlib, MakeComparingAdnl>;

type AdapterFactory = Either<SingleFactory, ComparingFactory>;

#[derive(Copy, Clone, Debug, PartialEq, Eq, ValueEnum)]
enum ClientImpl {
    Tonlibjson,
    AdnlTcp,
    ComparingTonlib,
    ComparingAdnl,
}

#[derive(clap::Args, Debug)]
#[group(multiple = false)]
struct TonConfigArgs {
    #[clap(long, value_parser = Url::parse)]
    ton_config_url: Option<Url>,
    #[clap(long)]
    ton_config_path: Option<PathBuf>,
}

impl From<TonConfigArgs> for ConfigSource {
    fn from(args: TonConfigArgs) -> Self {
        if let Some(path) = args.ton_config_path {
            return Self::File { path };
        };

        Self::Url {
            url: args.ton_config_url.unwrap_or(default_ton_config_url()),
            interval: Duration::from_secs(60),
        }
    }
}

#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
struct Args {
    #[clap(long, default_value = "0.0.0.0:50052")]
    listen: SocketAddr,
    #[clap(long, value_parser = humantime::parse_duration, default_value = "30s")]
    timeout: Duration,
    #[clap(long, value_parser = humantime::parse_duration, default_value = "300s")]
    tcp_keepalive: Duration,
    #[clap(long, value_parser = humantime::parse_duration, default_value = "120s")]
    http2_keepalive_interval: Duration,
    #[clap(long, value_parser = humantime::parse_duration, default_value = "20s")]
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
    ton_config: TonConfigArgs,
    #[clap(long, value_parser = humantime::parse_duration, default_value = "10s")]
    ton_timeout: Duration,
    #[clap(long, value_parser = humantime::parse_duration, default_value = "10s")]
    retry_budget_ttl: Duration,
    #[clap(long, default_value_t = 1)]
    retry_min_rps: u32,
    #[clap(long, default_value_t = 0.1)]
    retry_withdraw_percent: f32,
    #[clap(long, value_parser = humantime::parse_duration, default_value = "128ms")]
    retry_first_delay: Duration,
    #[clap(long, value_parser = humantime::parse_duration, default_value = "4096ms")]
    retry_max_delay: Duration,

    #[clap(long, value_parser = humantime::parse_duration, default_value = "70ms")]
    ewma_default_rtt: Duration,
    #[clap(long, value_parser = humantime::parse_duration, default_value = "1ms")]
    ewma_decay: Duration,
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
            .with_http_listener(args.metrics_listen)
            .install()
            .expect("failed to install Prometheus recorder");

        tracing::info!("Listening metrics on {:?}", &args.metrics_listen);
    }

    let config_source = ConfigSource::from(args.ton_config);
    match &config_source {
        ConfigSource::File { path } => tracing::info!("TON Config path: {}", path.display()),
        ConfigSource::Url { url, .. } => tracing::info!("TON Config URL: {}", url),
        ConfigSource::Config { .. } => tracing::info!("TON Config: inline"),
    }
    tracing::info!("Client implementation: {:?}", &args.client);

    let factory: AdapterFactory = match args.client {
        ClientImpl::Tonlibjson => {
            let f: MakeTonlibjsonSingle =
                MakeTonlibjsonAdapter.map_response((|a| Either::Left(Either::Left(a))) as _);
            Either::Left(Either::Left(f))
        }
        ClientImpl::AdnlTcp => {
            let f: MakeLiteServerSingle =
                MakeLiteServerAdapter.map_response((|a| Either::Left(Either::Right(a))) as _);
            Either::Left(Either::Right(f))
        }
        ClientImpl::ComparingTonlib => {
            let f: MakeComparingTonlib =
                MakeComparingAdapter::new(MakeTonlibjsonAdapter, MakeLiteServerAdapter)
                    .map_response((|a| Either::Right(Either::Left(a))) as _);
            Either::Right(Either::Left(f))
        }
        ClientImpl::ComparingAdnl => {
            let f: MakeComparingAdnl =
                MakeComparingAdapter::new(MakeLiteServerAdapter, MakeTonlibjsonAdapter)
                    .map_response((|a| Either::Right(Either::Right(a))) as _);
            Either::Right(Either::Right(f))
        }
    };

    let mut client =
        TonClientBuilder::<AdapterFactory>::with_factory_and_source(factory, config_source)
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
        .set_serving::<AccountServiceServer<AccountService<Client>>>()
        .await;
    health_reporter
        .set_serving::<BlockServiceServer<BlockService<Client>>>()
        .await;
    health_reporter
        .set_serving::<MessageServiceServer<MessageService<Client>>>()
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
