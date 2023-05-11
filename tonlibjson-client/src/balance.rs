use tower::{Service, ServiceExt};
use tower::discover::{Change, Discover, ServiceList};
use tower::load::Load;
use std::{pin::Pin, task::{Context, Poll}};
use std::collections::HashMap;
use std::future::Future;
use anyhow::anyhow;
use derive_new::new;
use futures::{FutureExt, ready, StreamExt};
use tracing::{debug};
use futures::TryFutureExt;
use crate::session::SessionRequest;
use crate::discover::CursorClientDiscover;
use itertools::Itertools;
use tokio::select;
use tokio_stream::StreamMap;
use tokio_stream::wrappers::WatchStream;
use crate::block::{BlockHeader};
use crate::cursor_client::{CursorClient};

#[derive(Debug, Clone, Copy)]
pub enum Route {
    Any,
    Block { chain: i32, criteria: BlockCriteria },
    Latest { chain: i32 }
}

#[derive(Debug, Clone, Copy)]
pub enum BlockCriteria {
    Seqno(i32),
    LogicalTime(i64)
}

impl Route {
    pub fn choose(&self, services: &HashMap<String, CursorClient>) -> Vec<CursorClient> {
        return match self {
            Route::Any => { services.values().cloned().collect() },
            Route::Block { chain, criteria} => {
                services.values()
                    .filter_map(|s| s.load().map(|m| (s, m)))
                    .filter(|(_, metrics)| {
                        let (first_block, last_block) = match chain {
                            -1 => (&metrics.first_block.0, &metrics.last_block.0),
                            _ => (&metrics.first_block.1, &metrics.last_block.1)
                        };

                        match criteria {
                            BlockCriteria::LogicalTime(lt) => first_block.start_lt <= *lt && *lt <= last_block.end_lt,
                            BlockCriteria::Seqno(seqno) => first_block.id.seqno <= *seqno && *seqno <= last_block.id.seqno
                        }
                    })
                    .map(|(s, _)| s)
                    .cloned()
                    .collect()
            },
            Route::Latest { chain } => {
                let groups = services.values()
                    .filter_map(|s| s.load().map(|m| (s, m)))
                    .sorted_by_key(|(_, metrics)| match chain {
                        -1 => -metrics.last_block.0.id.seqno,
                        _ => -metrics.last_block.1.id.seqno
                    })
                    .group_by(|(_, metrics)| match chain {
                        -1 => metrics.last_block.0.id.seqno,
                        _ => metrics.last_block.1.id.seqno
                    });


                let mut idxs = vec![];
                for (_, group) in &groups {
                    idxs = group.collect();

                    // we need at least 3 nodes in group
                    if idxs.len() > 2 {
                        break;
                    }
                }

                idxs.into_iter().map(|(s, _)| s).cloned().collect()
            }
        }
    }
}

pub struct Router {
    discover: CursorClientDiscover,
    services: HashMap<String, CursorClient>,
    pub last_headers: BlockChannel,
    ready: bool
}

impl Router {
    pub fn new(discover: CursorClientDiscover) -> Self {
        Router {
            discover,
            services: HashMap::new(),
            last_headers: BlockChannel::new(),
            ready: false
        }
    }

    pub fn last_block_receiver(&self) -> tokio::sync::broadcast::Receiver<(BlockHeader, BlockHeader)> {
        self.last_headers.receiver()
    }

    fn update_pending_from_discover(
        &mut self,
        cx: &mut Context<'_>,
    ) -> Poll<Option<Result<(), anyhow::Error>>> {
        debug!("updating from discover");
        loop {
            match ready!(Pin::new(&mut self.discover).poll_discover(cx))
                .transpose()
                .map_err(|e| anyhow!(e))?
            {
                None => return Poll::Ready(None),
                Some(Change::Remove(key)) => {
                    self.services.remove(&key);
                    self.last_headers.remove(key);
                }
                Some(Change::Insert(key, svc)) => {
                    self.last_headers.insert(key.clone(), svc.last_block_rx.clone());
                    self.services.insert(key, svc);
                }
            }
        }
    }
}

impl Service<Route> for Router {
    type Response = tower::balance::p2c::Balance<ServiceList<Vec<CursorClient>>, SessionRequest>;
    type Error = anyhow::Error;
    type Future = Pin<Box<dyn Future<Output = Result<Self::Response, Self::Error>> + Send>>;

    fn poll_ready(&mut self, cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        let _ = self.update_pending_from_discover(cx)?;

        if self.ready {
            return Poll::Ready(Ok(()));
        }

        // wait once
        if self.services.len() > 0 && self.services.values().all(|svc| svc.load().is_some()) {
            for s in self.services.values() {
                let m = s.load().expect("service must be ready");
                tracing::info!(metrics = m.first_block.0.id.seqno, "service ready");
            }

            self.ready = true;

            return Poll::Ready(Ok(()));
        }

        cx.waker().wake_by_ref();

        Poll::Pending
    }

    fn call(&mut self, req: Route) -> Self::Future {
        let services = req.choose(&self.services);

        async move {
            if services.is_empty() {
                Err(anyhow!("no services available for {:?}", req))
            } else {
                Ok(tower::balance::p2c::Balance::new(ServiceList::new(services)))
            }
        }.boxed()
    }
}


#[derive(new)]
pub struct BalanceRequest {
    pub route: Route,
    pub request: SessionRequest
}

pub struct Balance {
    router: Router
}

impl Balance {
    pub fn new(router: Router) -> Self {
        Balance { router }
    }
}

impl Service<BalanceRequest> for Balance {
    type Response = <<CursorClientDiscover as Discover>::Service as Service<SessionRequest>>::Response;
    type Error = anyhow::Error;
    type Future = Pin<Box<dyn Future<Output = Result<Self::Response, Self::Error>> + Send>>;

    fn poll_ready(&mut self, cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        self.router.poll_ready(cx).map_err(|e| anyhow!(e))
    }

    fn call(&mut self, request: BalanceRequest) -> Self::Future {
        let (route, request) = (request.route, request.request);

        self.router.call(route).and_then(|mut svc| async move {
            svc.ready()
                .await
                .map_err(|e| anyhow!(e))?
                .call(request)
                .await
                .map_err(|e| anyhow!(e))
        }).boxed()
    }
}


type BlockChannelItem = (BlockHeader, BlockHeader);

enum BlockChannelChange {
    Insert { key: String, watcher: tokio::sync::watch::Receiver<Option<BlockChannelItem>>},
    Remove { key: String }
}

pub struct BlockChannel {
    changes: tokio::sync::mpsc::UnboundedSender<BlockChannelChange>,
    joined: tokio::sync::broadcast::Receiver<BlockChannelItem>
}

impl BlockChannel {
    pub fn new() -> Self {
        let (tx, mut rx) = tokio::sync::mpsc::unbounded_channel::<BlockChannelChange>();
        let (tj, rj) = tokio::sync::broadcast::channel::<BlockChannelItem>(256);

        tokio::spawn(async move {
            let mut stream_map = StreamMap::new();

            let mut last_seqno = 0;

            loop {
                select! {
                    Some(change) = rx.recv() => {
                        match change {
                            BlockChannelChange::Insert { key, watcher } => { stream_map.insert(key, WatchStream::from_changes(watcher)); },
                            BlockChannelChange::Remove { key } => { stream_map.remove(&key); }
                        }
                    },
                    Some((_, Some((master, worker)))) = stream_map.next() => {
                        if master.id.seqno > last_seqno {
                            last_seqno = master.id.seqno;

                            let _ = tj.send((master, worker));
                        }
                    }
                };
            }
        });

        Self {
            changes: tx,
            joined: rj
        }
    }

    pub fn insert(&self, key: String, watcher: tokio::sync::watch::Receiver<Option<BlockChannelItem>>) {
        let _ = self.changes.send(BlockChannelChange::Insert { key, watcher });
    }

    pub fn remove(&self, key: String) {
        let _ = self.changes.send(BlockChannelChange::Remove { key });
    }

    pub fn receiver(&self) -> tokio::sync::broadcast::Receiver<BlockChannelItem> {
        self.joined.resubscribe()
    }
}
