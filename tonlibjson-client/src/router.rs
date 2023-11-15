use std::pin::Pin;
use std::task::{Context, Poll, ready};
use futures::future::BoxFuture;
use futures::{FutureExt, StreamExt};
use tower::discover::{Change, Discover};
use tower::Service;
use anyhow::anyhow;
use dashmap::DashMap;
use itertools::Itertools;
use tokio::select;
use tokio_stream::StreamMap;
use tokio_stream::wrappers::WatchStream;
use crate::block::MasterchainInfo;
use crate::cursor_client::CursorClient;
use crate::discover::CursorClientDiscover;

pub(crate) trait Routable {
    fn route(&self) -> Route;
}

pub(crate) struct Router {
    discover: CursorClientDiscover,
    services: DashMap<String, CursorClient>,
    last_block: MergeStreamMap<MasterchainInfo>
}

impl Router {
    pub(crate) fn new(discover: CursorClientDiscover) -> Self {
        metrics::describe_counter!("ton_router_miss_count", "Count of misses in router");
        metrics::describe_counter!("ton_router_delayed_count", "Count of delayed requests in router");
        metrics::describe_counter!("ton_router_delayed_hit_count", "Count of delayed request hits in router");
        metrics::describe_counter!("ton_router_delayed_miss_count", "Count of delayed request misses in router");

        Router {
            discover,
            services: Default::default(),
            last_block: MergeStreamMap::new()
        }
    }

    fn update_pending_from_discover(&mut self, cx: &mut Context<'_>, ) -> Poll<Option<Result<(), anyhow::Error>>> {
        loop {
            match ready!(Pin::new(&mut self.discover).poll_discover(cx)).transpose()? {
                None => return Poll::Ready(None),
                Some(Change::Remove(key)) => {
                    self.services.remove(&key);
                    self.last_block.remove(&key);
                }
                Some(Change::Insert(key, svc)) => {
                    self.last_block.insert(key.clone(), svc.subscribe_masterchain_info());
                    self.services.insert(key, svc);
                }
            }
        }
    }

    fn distance_to(&self, chain: &i32, criteria: &BlockCriteria) -> Option<i32> {
        self.services
            .iter()
            .filter_map(|s| s.distance_to(chain, criteria))
            .filter(|d| d.is_positive())
            .min()
    }
}

#[derive(Debug, Clone, Copy)]
pub(crate) enum BlockCriteria {
    Seqno { shard: i64, seqno: i32 },
    LogicalTime(i64)
}

#[derive(Debug, Clone, Copy)]
pub(crate) enum Route {
    Block { chain: i32, criteria: BlockCriteria },
    Latest
}

impl Service<&Route> for Router {
    type Response = Vec<CursorClient>;
    type Error = anyhow::Error;
    type Future = BoxFuture<'static, Result<Self::Response, Self::Error>>;

    fn poll_ready(&mut self, cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        let _ = self.update_pending_from_discover(cx)?;

        if self.services.iter().any(|s| s.edges_defined()) {
            Poll::Ready(Ok(()))
        } else {
            cx.waker().wake_by_ref();

            Poll::Pending
        }
    }

    fn call(&mut self, req: &Route) -> Self::Future {
        let services = req.choose(&self.services);
        if !services.is_empty() {
            return std::future::ready(Ok(services)).boxed();
        }

        metrics::increment_counter!("ton_router_miss_count");

        if let Route::Block { chain, criteria } = req {
            if self.distance_to(chain, criteria).is_some_and(|d| d <= 1) {
                metrics::increment_counter!("ton_router_delayed_count");

                let req = *req;
                let svcs = self.services.clone();
                let mut next_block = self.last_block.receiver();

                return async move {
                    loop {
                        let _ = next_block.recv().await?;

                        let services = req.choose(&svcs);
                        if !services.is_empty() {
                            metrics::increment_counter!("ton_router_delayed_hit_count");

                            return Ok(services)
                        }
                    }
                }.boxed();
            }
        }

        std::future::ready(Err(anyhow!("no services available for {:?}", req))).boxed()
    }
}


impl Route {
    fn choose(&self, services: &DashMap<String, CursorClient>) -> Vec<CursorClient> {
        match self {
            Route::Block { chain, criteria } => {
                services
                    .iter()
                    .filter(|s| s.contains(chain, criteria))
                    .map(|s| s.clone())
                    .collect()
            },
            Route::Latest => {
                let groups = services
                    .iter()
                    .filter_map(|s| s.last_seqno().map(|seqno| (s, seqno)))
                    .sorted_unstable_by_key(|(_, seqno)| -seqno)
                    .group_by(|(_, seqno)| *seqno);

                let mut idxs = vec![];
                for (_, group) in &groups {
                    idxs = group.collect();

                    // we need at least 3 nodes in group
                    if idxs.len() > 2 {
                        break;
                    }
                }

                idxs.into_iter()
                    .map(|(s, _)| s.clone())
                    .collect()
            }
        }
    }
}


enum StreamMapChange<T> {
    Insert { key: String, stream: WatchStream<Option<T>>},
    Remove { key: String }
}
struct MergeStreamMap<T> {
    changes: tokio::sync::mpsc::UnboundedSender<StreamMapChange<T>>,
    joined: tokio::sync::broadcast::Receiver<T>
}

impl<T> MergeStreamMap<T> where T : Sync + Send + Clone + Ord + 'static {
    fn new() -> Self {
        let (changes, mut rx) = tokio::sync::mpsc::unbounded_channel::<StreamMapChange<T>>();
        let (tj, joined) = tokio::sync::broadcast::channel::<T>(256);

        tokio::spawn(async move {
            let mut stream_map = StreamMap::new();
            let mut last_seqno = None;

            loop { select! {
                Some(change) = rx.recv() => { match change {
                    StreamMapChange::Insert { key, stream } => { stream_map.insert(key, stream); },
                    StreamMapChange::Remove { key } => { stream_map.remove(&key); }
                }},
                Some((_, Some(master))) = stream_map.next() => {
                   if last_seqno.is_none() || last_seqno.as_ref().is_some_and(|last_seqno| &master > last_seqno) {
                        last_seqno.replace(master.clone());

                        let _ = tj.send(master);
                    }
                }
            } }
        });

        Self { changes, joined }
    }

    fn insert(&self, key: String, watcher: tokio::sync::watch::Receiver<Option<T>>) {
        let _ = self.changes.send(StreamMapChange::Insert { key, stream: WatchStream::from_changes(watcher) });
    }

    fn remove(&self, key: &str) {
        let _ = self.changes.send(StreamMapChange::Remove { key: key.to_owned() });
    }

    fn receiver(&self) -> tokio::sync::broadcast::Receiver<T> {
        self.joined.resubscribe()
    }
}
