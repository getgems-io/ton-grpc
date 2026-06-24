use crate::{
    Client, RoutedClient, TonService,
    pool::{Balance, LiteServerDiscoverError, LiteServerDiscoverHandle},
};
use futures::{Stream, StreamExt, TryStreamExt, stream};
use std::{future::ready, path::PathBuf, pin::Pin, time::Duration};
use tokio::time::{Interval, MissedTickBehavior};
use tokio_stream::wrappers::IntervalStream;
use ton_config::{
    LiteServerId, TonConfig, default_ton_config_url, load_ton_config, read_ton_config,
};
use ton_tower::{
    request::GetMasterchainInfo,
    service::{
        error::{ErrorLayer, ErrorService},
        metric::ConcurrencyMetric,
        reconnect::Reconnect,
        retry::RetryPolicy,
        shared::{SharedLayer, SharedService},
        timeout::{Timeout, TimeoutLayer},
    },
};
use tower::{
    Service, ServiceBuilder,
    discover::Change,
    limit::{ConcurrencyLimit, ConcurrencyLimitLayer, RateLimit},
    load::{CompleteOnResponse, PeakEwma, PeakEwmaDiscover},
    retry::{Retry, RetryLayer, budget::TpsBudget},
    util::Either,
};
use url::Url;

pub type ReconnectingClient<F> = Reconnect<RateLimit<F>, TonConfig>;

pub type WrappedCursor<F> = RoutedClient<
    ConcurrencyMetric<
        ConcurrencyLimit<SharedService<ErrorService<Timeout<PeakEwma<ReconnectingClient<F>>>>>>,
    >,
>;

pub type BoxClientDiscover<F> = Pin<
    Box<
        dyn Stream<Item = Result<Change<LiteServerId, WrappedCursor<F>>, LiteServerDiscoverError>>
            + Send,
    >,
>;

pub type SharedBalance<F> = SharedService<Balance<WrappedCursor<F>, BoxClientDiscover<F>>>;

pub type PoolTransport<F> =
    ErrorService<Timeout<Either<Retry<RetryPolicy, SharedBalance<F>>, SharedBalance<F>>>>;

#[derive(Debug)]
pub enum ConfigSource {
    File { path: PathBuf },
    Url { url: Url, interval: Duration },
    Config { config: TonConfig },
}

pub struct TonClientBuilder<F> {
    factory: F,
    config_source: ConfigSource,
    timeout: Duration,
    ewma_default_rtt: Duration,
    ewma_decay: Duration,
    retry_enabled: bool,
    retry_budget_ttl: Duration,
    retry_min_per_sec: u32,
    retry_percent: f32,
    retry_first_delay: Duration,
    retry_max_delay: Duration,
}

impl<F: Default> Default for TonClientBuilder<F> {
    fn default() -> Self {
        Self::from_config_url(default_ton_config_url(), Duration::from_secs(60))
    }
}

impl<F: Default> TonClientBuilder<F> {
    pub fn from_config_path(path: PathBuf) -> Self {
        Self::with_factory_and_source(F::default(), ConfigSource::File { path })
    }

    pub fn from_config_url(url: Url, interval: Duration) -> Self {
        Self::with_factory_and_source(F::default(), ConfigSource::Url { url, interval })
    }

    pub fn from_config(config: &TonConfig) -> Self {
        Self::with_factory_and_source(
            F::default(),
            ConfigSource::Config {
                config: config.clone(),
            },
        )
    }

    pub fn from_config_source(config_source: impl Into<ConfigSource>) -> Self {
        Self::with_factory_and_source(F::default(), config_source.into())
    }
}

impl<F> TonClientBuilder<F> {
    pub fn with_factory_and_source(factory: F, config_source: ConfigSource) -> Self {
        Self {
            factory,
            config_source,
            timeout: Duration::from_secs(10),
            ewma_default_rtt: Duration::from_millis(70),
            ewma_decay: Duration::from_millis(1),
            retry_enabled: true,
            retry_budget_ttl: Duration::from_secs(10),
            retry_min_per_sec: 10,
            retry_percent: 0.1,
            retry_first_delay: Duration::from_millis(128),
            retry_max_delay: Duration::from_millis(4096),
        }
    }

    pub fn set_ewma_default_rtt(mut self, default_rtt: Duration) -> Self {
        self.ewma_default_rtt = default_rtt;
        self
    }

    pub fn set_ewma_decay(mut self, decay: Duration) -> Self {
        self.ewma_decay = decay;
        self
    }

    pub fn disable_retry(mut self) -> Self {
        self.retry_enabled = false;
        self
    }

    pub fn set_retry_budget_ttl(mut self, budget_ttl: Duration) -> Self {
        self.retry_budget_ttl = budget_ttl;
        self
    }

    pub fn set_retry_min_per_sec(mut self, retry_min_per_sec: u32) -> Self {
        self.retry_min_per_sec = retry_min_per_sec;
        self
    }

    pub fn set_retry_percent(mut self, retry_percent: f32) -> Self {
        self.retry_percent = retry_percent;
        self
    }

    pub fn set_retry_first_delay(mut self, first_delay: Duration) -> Self {
        self.retry_first_delay = first_delay;
        self
    }

    pub fn set_retry_max_delay(mut self, delay: Duration) -> Self {
        self.retry_max_delay = delay;
        self
    }

    pub fn set_timeout(mut self, timeout: Duration) -> Self {
        self.timeout = timeout;
        self
    }

    pub fn build(self) -> anyhow::Result<Client<PoolTransport<F>>>
    where
        F: Service<TonConfig, Response: TonService, Error: Send + Sync, Future: Send + Unpin>
            + Clone
            + Send
            + 'static,
        anyhow::Error: From<F::Error>,
    {
        tracing::debug!(config_source = ?self.config_source);
        let stream: Pin<Box<dyn Stream<Item = Result<TonConfig, anyhow::Error>> + Send>> =
            match self.config_source {
                ConfigSource::File { path } => {
                    let mut interval = tokio::time::interval(Duration::from_secs(1));
                    interval.set_missed_tick_behavior(MissedTickBehavior::Skip);
                    read_ton_config_from_file_stream(path, interval).boxed()
                }
                ConfigSource::Url { url, interval } => {
                    let mut interval = tokio::time::interval(interval);
                    interval.set_missed_tick_behavior(MissedTickBehavior::Skip);
                    read_ton_config_from_url_stream(url, interval).boxed()
                }
                ConfigSource::Config { config } => stream::once(ready(Ok(config))).boxed(),
            };
        let lite_server_discover = LiteServerDiscoverHandle::new(stream);
        let factory = self.factory;
        let client_discover = lite_server_discover.map_ok(move |change| {
            let mk = ServiceBuilder::new()
                .rate_limit(1, Duration::from_secs(60))
                .service(factory.clone());

            match change {
                Change::Insert(k, config) => Change::Insert(k, Reconnect::new(mk, config)),
                Change::Remove(k) => Change::Remove(k),
            }
        });

        let ewma_discover = PeakEwmaDiscover::new::<GetMasterchainInfo>(
            client_discover,
            self.ewma_default_rtt,
            self.ewma_decay,
            CompleteOnResponse::default(),
        );

        let cursor_client_discover = ewma_discover
            .map_ok(|s| match s {
                Change::Insert(k, v) => {
                    let svc = ServiceBuilder::new()
                        .layer_fn(|svc| RoutedClient::new(k.to_string(), svc))
                        .layer_fn(|svc| ConcurrencyMetric::new(svc, k.to_string()))
                        .layer(ConcurrencyLimitLayer::new(256))
                        .layer(SharedLayer)
                        .layer(ErrorLayer)
                        .layer(TimeoutLayer::new(Duration::from_secs(5)))
                        .service(v);

                    Change::Insert(k, svc)
                }
                Change::Remove(k) => Change::Remove(k),
            })
            .boxed();

        let svc = ServiceBuilder::new()
            .layer_fn(Client::new)
            .layer(ErrorLayer)
            .layer(TimeoutLayer::new(self.timeout))
            .option_layer(self.retry_enabled.then(|| {
                RetryLayer::new(RetryPolicy::new(
                    TpsBudget::new(
                        self.retry_budget_ttl,
                        self.retry_min_per_sec,
                        self.retry_percent,
                    ),
                    self.retry_first_delay.as_millis() as u64,
                    self.retry_max_delay,
                ))
            }))
            .layer(SharedLayer)
            .service(Balance::new(cursor_client_discover));

        Ok(svc)
    }
}

pub fn read_ton_config_from_file_stream(
    path: PathBuf,
    interval: Interval,
) -> impl Stream<Item = Result<TonConfig, anyhow::Error>> {
    IntervalStream::new(interval)
        .map(move |_| path.clone())
        .then(read_ton_config)
}

pub fn read_ton_config_from_url_stream(
    url: Url,
    interval: Interval,
) -> impl Stream<Item = Result<TonConfig, anyhow::Error>> {
    IntervalStream::new(interval)
        .map(move |_| url.clone())
        .then(load_ton_config)
}
