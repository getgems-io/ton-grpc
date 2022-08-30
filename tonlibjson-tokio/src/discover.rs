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
use std::future;

use crate::liteserver::{extract_liteserver_list, load_ton_config, Liteserver};
use crate::{build_client, AsyncClient, ServiceError};
use tokio_stream::Stream;
use tower::discover::Change;
use tracing::{debug, error};

use futures::stream::{StreamExt};
use tokio::time::MissedTickBehavior::Skip;

type DiscoverResult<K, S, E> = Result<Change<K, S>, E>;

pub struct DynamicServiceStream {
    changes: Receiver<Change<String, AsyncClient>>,
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
    type Item = DiscoverResult<String, AsyncClient, ServiceError>;

    fn poll_next(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        let c = &mut self.changes;
        match Pin::new(&mut *c).poll_recv(cx) {
            Poll::Pending | Poll::Ready(None) => Poll::Pending,
            Poll::Ready(Some(change)) => match change {
                Change::Insert(k, client) => Poll::Ready(Some(Ok(Change::Insert(k, client)))),
                Change::Remove(k) => Poll::Ready(Some(Ok(Change::Remove(k)))),
            },
        }
    }
}

impl Unpin for DynamicServiceStream {}

async fn changes(
    url: Url,
    liteservers: &HashSet<Liteserver>,
    tx: Sender<Change<String, AsyncClient>>,
) -> anyhow::Result<HashSet<Liteserver>> {
    let config = load_ton_config(url).await?;
    let liteserver_new = extract_liteserver_list(&config)?;
    let config_parsed: Value = serde_json::from_str(&config)?;

    let lsn = liteserver_new.clone();

    for ls in liteservers.difference(&liteserver_new) {
        debug!("remove {:?}", ls.identifier());
        tx.send(Change::Remove(ls.identifier())).await?;
    }

    tokio_stream::iter(lsn.difference(liteservers))
        .then(|ls| {
            let mut config = config_parsed.clone();
            async move {
                let id = ls.identifier();
                debug!("found new liteserver {:?}", id);
                let ls = serde_json::to_value(ls)?;
                config["liteservers"] = Value::Array(vec![ls]);

                anyhow::Ok((id, config))
        }})
        .filter_map(|f| async {
            match f {
                Err(e) => {
                    error!("{:?}", e);
                    None
                },
                Ok(v) => Some(v)
            }
        })
        .then(|(id, config)| async move {
            async move {
                (id, build_client(&config).await)
            }
        })
        .buffer_unordered(12)
        .filter(|(_, client)| {
            future::ready(client.is_ok())
        })
        .for_each(|(id, client)| async {
            let _ = tx.send(Change::Insert(id, client.unwrap())).await;
        }).await;

    Ok(liteserver_new)
}
