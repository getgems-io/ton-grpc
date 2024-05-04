use crate::make::{CursorClientFactory, ClientFactory};
use crate::ton_config::{load_ton_config, read_ton_config, TonConfig};
use reqwest::Url;
use std::time::Duration;
use std::{
    pin::Pin,
    task::{Context, Poll},
};
use std::collections::HashSet;
use std::net::IpAddr;
use std::path::PathBuf;
use futures::{TryStreamExt, StreamExt};
use tokio_stream::{Stream};
use tower::discover::Change;
use tower::load::PeakEwmaDiscover;
use tower::ServiceExt;
use hickory_resolver::system_conf::read_system_conf;
use hickory_resolver::TokioAsyncResolver;
use tokio::sync::mpsc::{UnboundedReceiver, UnboundedSender};
use tokio::time::{Interval, MissedTickBehavior};
use tokio_stream::wrappers::IntervalStream;
use crate::client::Client;
use crate::cursor_client::CursorClient;
use crate::ton_config::Liteserver;

type DiscoverResult<C> = Result<Change<String, C>, anyhow::Error>;

pub(crate) struct ClientDiscover {
    rx: UnboundedReceiver<DiscoverResult<Client>>,
}

fn read_ton_config_from_file_stream(path: PathBuf, interval: Interval) -> impl Stream<Item = Result<TonConfig, anyhow::Error>> {
    IntervalStream::new(interval)
        .map(move |_| { path.clone() })
        .then(read_ton_config)
}

fn read_ton_config_from_url_stream(url: Url, interval: Interval) -> impl Stream<Item = Result<TonConfig, anyhow::Error>> {
    IntervalStream::new(interval)
        .map(move |_| { url.clone() })
        .then(load_ton_config)
}

impl ClientDiscover {
    pub(crate) async fn from_path(path: PathBuf, /* interval: Duration */) -> anyhow::Result<Self> {
        let mut interval = tokio::time::interval(Duration::from_secs(1));
        interval.set_missed_tick_behavior(MissedTickBehavior::Skip);

        let stream = read_ton_config_from_file_stream(path, interval);

        let (tx, rx) = tokio::sync::mpsc::unbounded_channel();
        tokio::spawn(Self::inner(tx, stream));

        Ok(Self { rx })
    }

    pub(crate) async fn new(url: Url, period: Duration) -> anyhow::Result<Self> {
        let mut interval = tokio::time::interval(period);
        interval.set_missed_tick_behavior(MissedTickBehavior::Skip);

        let stream = read_ton_config_from_url_stream(url.clone(), interval);

        let (tx, rx) = tokio::sync::mpsc::unbounded_channel();
        tokio::spawn(Self::inner(tx, stream));

        Ok(Self { rx })
    }

    async fn inner(tx: UnboundedSender<DiscoverResult<Client>>, stream: impl Stream<Item = Result<TonConfig, anyhow::Error>>) {
        tokio::pin!(stream);
        let mut factory = ClientFactory;
        let mut liteservers = HashSet::default();
        let dns = Self::dns_resolver();

        while let Ok(Some(new_config)) = stream.try_next().await {
            tracing::info!("tick service discovery");

            let mut liteserver_new: HashSet<Liteserver> = HashSet::default();
            for ls in new_config.liteservers.iter() {
                match Self::apply_dns(dns.clone(), ls.clone()).await {
                    Err(e) => tracing::error!("dns error: {:?}", e),
                    Ok(ls) => { liteserver_new.insert(ls); }
                }
            }

            let remove = liteservers.difference(&liteserver_new)
                .collect::<Vec<&Liteserver>>();
            let insert = liteserver_new.difference(&liteservers)
                .collect::<Vec<&Liteserver>>();

            tracing::info!("Discovered {} liteservers, remove {}, insert {}", liteserver_new.len(), remove.len(), insert.len());
            for ls in liteservers.difference(&liteserver_new) {
                tracing::info!("remove {:?}", ls.id());
                let _ = tx.send(Ok(Change::Remove(ls.id())));
            }

            for ls in liteserver_new.difference(&liteservers) {
                tracing::info!("insert {:?}", ls.id());

                if let Ok(client) = (&mut factory).oneshot(new_config.with_liteserver(ls)).await {
                    let _ = tx.send(Ok(Change::Insert(ls.id(), client)));
                }
            }

            liteservers.clone_from(&liteserver_new);
        }
    }

    async fn apply_dns(dns_resolver: TokioAsyncResolver, ls: Liteserver) -> anyhow::Result<Liteserver> {
        if let Some(host) = &ls.host {
            let records = dns_resolver.lookup_ip(host).await?;

            for record in records {
                if let IpAddr::V4(ip) = record {
                    return Ok(ls.with_ip(Into::<u32>::into(ip) as i32));
                }
            }
        }

        Ok(ls)
    }

    fn dns_resolver() -> TokioAsyncResolver {
        let (resolver_config, mut resolver_opts) = read_system_conf().unwrap();
        resolver_opts.positive_max_ttl = Some(Duration::from_secs(1));
        resolver_opts.negative_max_ttl = Some(Duration::from_secs(1));

        TokioAsyncResolver::tokio(resolver_config, resolver_opts)
    }
}

impl Stream for ClientDiscover {
    type Item = DiscoverResult<Client>;

    fn poll_next(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        match self.rx.poll_recv(cx) {
            Poll::Ready(Some(Ok(change))) => Poll::Ready(Some(Ok(change))),
            _ => Poll::Pending
        }
    }
}

pub(crate) struct CursorClientDiscover {
    discover: PeakEwmaDiscover<ClientDiscover>
}

impl CursorClientDiscover {
    pub(crate) fn new(discover: PeakEwmaDiscover<ClientDiscover>) -> Self {
        Self { discover }
    }
}

impl Stream for CursorClientDiscover {
    type Item = DiscoverResult<CursorClient>;

    fn poll_next(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        let c = &mut self.discover;
        match Pin::new(&mut *c).poll_next(cx) {
            Poll::Ready(Some(Ok(change))) => match change {
                Change::Insert(k, client) => Poll::Ready(Some(Ok(
                    Change::Insert(k.clone(), CursorClientFactory::create(k, client))
                ))),
                Change::Remove(k) => Poll::Ready(Some(Ok(Change::Remove(k)))),
            },
            _ => Poll::Pending
        }
    }
}
