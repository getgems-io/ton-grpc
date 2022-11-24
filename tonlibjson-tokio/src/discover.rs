use crate::make::ClientFactory;
use crate::ton_config::{load_ton_config, read_ton_config, TonConfig};
use async_stream::try_stream;
use reqwest::Url;
use std::time::Duration;
use std::{
    pin::Pin,
    task::{Context, Poll},
};
use std::collections::HashSet;
use std::path::PathBuf;
use futures::TryFutureExt;
use tokio_stream::Stream;
use tower::discover::Change;
use tower::limit::ConcurrencyLimit;
use tracing::{debug, info};
use tower::ServiceExt;
use tower::Service;
use crate::ton_config::Liteserver;
use crate::session::SessionClient;

type DiscoverResult<K, S, E> = Result<Change<K, S>, E>;

pub struct DynamicServiceStream {
    changes: Pin<Box<dyn Stream<Item = Result<Change<String, ConcurrencyLimit<SessionClient>>, anyhow::Error>> + Send>>,
}

impl DynamicServiceStream {
    pub(crate) async fn new(url: Url, period: Duration, fallback_path: Option<PathBuf>) -> anyhow::Result<Self> {
        let mut factory = ClientFactory::default();
        let mut interval = tokio::time::interval(period);
        let mut config = config(url.clone(), fallback_path).await?;

        let stream = try_stream! {
            interval.tick().await;
            info!("first tick service discovery");

            for ls in config.liteservers.iter() {
                if let Ok(client) = factory.ready().await?.call(config.with_liteserver(ls)).await {
                    yield Change::Insert(ls.id(), client);
                }
            }

            let mut liteservers = HashSet::from_iter(config.liteservers.iter().cloned());

            loop {
                interval.tick().await;
                info!("tick service discovery");

                let new_config = load_ton_config(url.clone()).await?;
                if new_config == config {
                    continue;
                }

                let liteserver_new: HashSet<Liteserver> = HashSet::from_iter(new_config.liteservers.iter().cloned());

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

                debug!("Discovered {} liteservers, remove {}, insert {}", liteserver_new.len(), remove.len(), insert.len());
                while !remove.is_empty() || !insert.is_empty() {
                    if let Some(ls) = remove.pop() {
                        debug!("remove {:?}", ls.id());
                        yield Change::Remove(ls.id());
                    }

                    if let Some(ls) = insert.pop() {
                        debug!("insert {:?}", ls.id());

                        if let Ok(client) = factory.ready().await?.call(new_config.with_liteserver(ls)).await {
                            yield Change::Insert(ls.id(), client);
                        }
                    }
                }

                liteservers = liteserver_new.clone();
                config = new_config;
            }
        };

        Ok(Self {
            changes: Box::pin(stream),
        })
    }
}

impl Stream for DynamicServiceStream {
    type Item = DiscoverResult<String, ConcurrencyLimit<SessionClient>, anyhow::Error>;

    fn poll_next(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        let c = &mut self.changes;
        match Pin::new(&mut *c).poll_next(cx) {
            Poll::Ready(Some(Ok(change))) => match change {
                Change::Insert(k, client) => Poll::Ready(Some(Ok(Change::Insert(
                    k,
                    client,
                )))),
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
