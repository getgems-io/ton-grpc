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
use tokio::sync::mpsc::UnboundedReceiver;
use tokio::time::{Interval, MissedTickBehavior};
use tokio_stream::wrappers::IntervalStream;
use crate::client::Client;
use crate::cursor_client::CursorClient;
use crate::ton_config::Liteserver;

// TODO[akostylev0] rework

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
        let mut factory = ClientFactory;

        let (tx, rx) = tokio::sync::mpsc::unbounded_channel();
        tokio::spawn(async move {
            tokio::pin!(stream);
            while let Ok(Some(config)) = stream.try_next().await {
                for ls in config.liteservers.iter() {
                    if let Ok(client) = (&mut factory).oneshot(config.with_liteserver(ls)).await {
                        let _ = tx.send(Ok(Change::Insert(ls.id(), client)));
                    }
                }
            }
        });

        Ok(Self { rx })
    }

    pub(crate) async fn new(url: Url, period: Duration, fallback_path: Option<PathBuf>) -> anyhow::Result<Self> {
        let mut interval = tokio::time::interval(period);
        interval.set_missed_tick_behavior(MissedTickBehavior::Skip);
        let mut factory = ClientFactory;
        let stream = read_ton_config_from_url_stream(url.clone(), interval);

        let (tx, rx) = tokio::sync::mpsc::unbounded_channel();

        tokio::spawn(async move {
            let dns = Self::dns_resolver();
            let mut liteservers = HashSet::default();
            tokio::pin!(stream);

            let mut config = None;
            while let Ok(Some(new_config)) = stream.try_next().await {
                tracing::info!("tick service discovery");

                let mut new_liteservers = vec![];
                for ls in new_config.liteservers.iter() {
                    let ls = if let Some(ls) = Self::dns_resolve(dns.clone(), ls.clone()).await { ls } else { ls.clone() };
                    new_liteservers.push(ls);
                }

                let liteserver_new: HashSet<Liteserver> = HashSet::from_iter(new_liteservers.iter().cloned());

                let (mut remove, mut insert) = if config.is_some_and(|c: TonConfig| c.data == new_config.data) {
                    (
                        liteservers.difference(&liteserver_new).collect::<Vec<&Liteserver>>(),
                        liteserver_new.difference(&liteservers).collect::<Vec<&Liteserver>>()
                    )
                } else {
                    (
                        liteservers.iter().collect::<Vec<&Liteserver>>(),
                        liteserver_new.iter().collect::<Vec<&Liteserver>>()
                    )
                };

                tracing::info!("Discovered {} liteservers, remove {}, insert {}", liteserver_new.len(), remove.len(), insert.len());
                while !remove.is_empty() || !insert.is_empty() {
                    if let Some(ls) = remove.pop() {
                        tracing::info!("remove {:?}", ls.id());
                        let _ = tx.send(Ok(Change::Remove(ls.id())));
                    }

                    if let Some(ls) = insert.pop() {
                        tracing::info!("insert {:?}", ls.id());

                        if let Ok(client) = (&mut factory).oneshot(new_config.with_liteserver(ls)).await {
                            let _ = tx.send(Ok(Change::Insert(ls.id(), client)));
                        }
                    }
                }

                liteservers = liteserver_new.clone();
                config = Some(new_config);
            }
        });

        Ok(Self { rx })
    }

    async fn dns_resolve(dns_resolver: TokioAsyncResolver, ls: Liteserver) -> Option<Liteserver> {
        if let Some(host) = &ls.host {
            let records = dns_resolver.lookup_ip(host).await.ok()?;

            for record in records {
                if let IpAddr::V4(ip) = record {
                    return Some(ls.with_ip(Into::<u32>::into(ip) as i32));
                }
            }
        }

        None
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
