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
use futures::{TryFutureExt};
use tokio_stream::{Stream};
use tower::discover::Change;
use tower::load::PeakEwmaDiscover;
use tower::ServiceExt;
use hickory_resolver::system_conf::read_system_conf;
use hickory_resolver::TokioAsyncResolver;
use tokio::sync::mpsc::UnboundedReceiver;
use crate::client::Client;
use crate::cursor_client::CursorClient;
use crate::ton_config::Liteserver;

// TODO[akostylev0] rework

type DiscoverResult<C> = Result<Change<String, C>, anyhow::Error>;

pub(crate) struct ClientDiscover {
    rx: UnboundedReceiver<DiscoverResult<Client>>,
}

impl ClientDiscover {
    pub(crate) async fn from_path(path: PathBuf) -> anyhow::Result<Self> {
        let config = read_ton_config(path).await?;
        let mut factory = ClientFactory;

        let (tx, rx) = tokio::sync::mpsc::unbounded_channel();
        tokio::spawn(async move {
            for ls in config.liteservers.iter() {
                if let Ok(client) = (&mut factory).oneshot(config.with_liteserver(ls)).await {
                    let _ = tx.send(Ok(Change::Insert(ls.id(), client)));
                }
            }
        });

        Ok(Self { rx })
    }

    pub(crate) async fn new(url: Url, period: Duration, fallback_path: Option<PathBuf>) -> anyhow::Result<Self> {
        let mut config = config(url.clone(), fallback_path).await?;
        let mut factory = ClientFactory;
        let mut interval = tokio::time::interval(period);

        let (tx, rx) = tokio::sync::mpsc::unbounded_channel();

        tokio::spawn(async move {
            let dns = Self::dns_resolver();

            interval.tick().await;
            tracing::info!("first tick service discovery");

            let mut liteservers = vec![];
            for ls in config.liteservers.iter() {
                let ls = if let Some(ls) = Self::dns_resolve(dns.clone(), ls.clone()).await { ls } else { ls.clone() };
                liteservers.push(ls);
            }

            for ls in &liteservers {
                if let Ok(client) = (&mut factory).oneshot(config.with_liteserver(&ls)).await {
                    tracing::info!("insert {:?}", ls.id());
                    let _ = tx.send(Ok(Change::Insert(ls.id(), client)));
                }
            }

            let mut liteservers = HashSet::from_iter(liteservers.iter().cloned());

            loop {
                interval.tick().await;
                tracing::info!("tick service discovery");

                let Ok(new_config) = load_ton_config(url.clone()).await else {
                    continue;
                };

                let mut new_liteservers = vec![];
                for ls in new_config.liteservers.iter() {
                    let ls = if let Some(ls) = Self::dns_resolve(dns.clone(), ls.clone()).await { ls } else { ls.clone() };
                    new_liteservers.push(ls);
                }

                let liteserver_new: HashSet<Liteserver> = HashSet::from_iter(new_liteservers.iter().cloned());

                let (mut remove, mut insert) = if config.data == new_config.data {
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
                config = new_config;
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

async fn config(url: Url, fallback_path: Option<PathBuf>) -> anyhow::Result<TonConfig> {
    load_ton_config(url).or_else(|e| async {
        if let Some(path) = fallback_path {
            return read_ton_config(path).await
        }

        Err(e)
    }).await
}
