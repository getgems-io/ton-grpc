use reqwest::Url;
use serde_json::Value;
use std::collections::HashSet;
use std::time::Duration;
use std::{
    pin::Pin,
    task::{Context, Poll},
    thread,
};
use tokio::runtime::Runtime;
use tokio::sync::mpsc::{channel, Receiver, Sender};
use crate::liteserver::{extract_liteserver_list, load_ton_config, Liteserver, LiteserverConfig};
use crate::{ClientFactory, ServiceError};
use tokio_stream::Stream;
use tower::discover::Change;
use tracing::{debug, error};
use tokio::time::MissedTickBehavior::Skip;
use tower::reconnect::Reconnect;
use crate::client::AsyncClient;

type DiscoverResult<K, S, E> = Result<Change<K, S>, E>;

pub struct DynamicServiceStream {
    changes: Receiver<Change<String, LiteserverConfig>>,
}

impl DynamicServiceStream {
    pub(crate) fn new(url: Url, period: Duration) -> Self {
        debug!("New service stream init");
        let (tx, rx) = channel(128);

        thread::spawn(move || {
            let rt = Runtime::new().unwrap();

            rt.block_on(async {
                debug!("spawn blocking loop for config reloading");

                let mut liteservers = HashSet::new();
                let mut interval = tokio::time::interval(period);
                interval.set_missed_tick_behavior(Skip);

                loop {
                    debug!("config reload tick");
                    match changes(url.clone(), &liteservers, tx.clone()).await {
                        Err(e) => {
                            error!("Error occured: {:?}", e);
                        }
                        Ok(new_liteservers) => {
                            liteservers = new_liteservers;
                        }
                    }

                    interval.tick().await;
                }
            });
        });

        Self { changes: rx }
    }
}

impl Stream for DynamicServiceStream {
    type Item = DiscoverResult<String, Reconnect<ClientFactory, LiteserverConfig>, ServiceError>;

    fn poll_next(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        let c = &mut self.changes;
        match Pin::new(&mut *c).poll_recv(cx) {
            Poll::Pending | Poll::Ready(None) => Poll::Pending,
            Poll::Ready(Some(change)) => match change {
                Change::Insert(k, client) => Poll::Ready(Some(Ok(Change::Insert(k, Reconnect::new::<AsyncClient, Value>(ClientFactory::default(), client))))),
                Change::Remove(k) => Poll::Ready(Some(Ok(Change::Remove(k)))),
            },
        }
    }
}

impl Unpin for DynamicServiceStream {}

async fn changes(
    url: Url,
    liteservers: &HashSet<Liteserver>,
    tx: Sender<Change<String, LiteserverConfig>>,
) -> anyhow::Result<HashSet<Liteserver>> {
    let config = load_ton_config(url).await?;
    let config: Value = serde_json::from_str(&config)?;
    let liteserver_new = extract_liteserver_list(&config)?;

    let liteservers_remove = liteservers.difference(&liteserver_new).collect::<Vec<&Liteserver>>();
    let liteservers_insert = liteserver_new.difference(liteservers).collect::<Vec<&Liteserver>>();

    debug!("Discovered {} liteservers, remove {}, insert {}", liteserver_new.len(), liteservers_remove.len(), liteservers_insert.len());

    for ls in liteservers_remove {
        debug!("remove {:?}", ls.identifier());
        tx.send(Change::Remove(ls.identifier())).await?;
    }

    for ls in liteservers_insert {
        debug!("insert {:?}", ls.identifier());

        tx.send(Change::Insert(ls.identifier(), LiteserverConfig::new(config.clone(), ls.clone()))).await?;
    }

    Ok(liteserver_new)
}
