use reqwest::Url;
use serde_json::Value;
use std::time::Duration;
use std::{
    pin::Pin,
    task::{Context, Poll}
};
use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};
use async_stream::stream;
use tokio_stream::Stream;
use tower::discover::Change;
use tracing::{error, info};
use tower::reconnect::Reconnect;
use crate::client::AsyncClient;
use crate::make::ClientFactory;
use crate::ton_config::{TonConfig, load_ton_config};

type DiscoverResult<K, S, E> = Result<Change<K, S>, E>;

pub struct DynamicServiceStream {
    changes: Pin<Box<dyn Stream<Item=Change<u64, TonConfig>> + Send>>
}

impl DynamicServiceStream {
    pub(crate) fn new(url: Url, period: Duration, size: u8) -> anyhow::Result<Self> {
        let mut interval = tokio::time::interval(period);
        let mut hash = 0;

        let stream = stream! {
            loop {
                interval.tick().await;

                info!("tick service discovery");
                let Ok(config) = load_ton_config(url.clone()).await else {
                    error!("cannot load config");
                    continue;
                };

                let mut hasher = DefaultHasher::new();
                config.hash(&mut hasher);
                let new_hash = hasher.finish();

                if new_hash != hash {
                    for i in 0..size {
                        info!("insert {}", new_hash + i as u64);
                        yield Change::Insert(new_hash + i as u64, config.clone());
                        info!("remove {}", hash + i as u64);
                        yield Change::Remove(hash + i as u64);
                    }

                    hash = new_hash
                }
            }
        };

        Ok(Self { changes: Box::pin(stream) })
    }
}

impl Stream for DynamicServiceStream {
    type Item = DiscoverResult<u64, Reconnect<ClientFactory, TonConfig>, anyhow::Error>;

    fn poll_next(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        let c = &mut self.changes;
        match Pin::new(&mut *c).poll_next(cx) {
            Poll::Pending | Poll::Ready(None) => Poll::Pending,
            Poll::Ready(Some(change)) => match change {
                Change::Insert(k, client) => Poll::Ready(Some(Ok(Change::Insert(k, Reconnect::new::<AsyncClient, Value>(ClientFactory::default(), client))))),
                Change::Remove(k) => Poll::Ready(Some(Ok(Change::Remove(k)))),
            },
        }
    }
}

impl Unpin for DynamicServiceStream {}
