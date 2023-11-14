use std::pin::Pin;
use std::task::{Context, Poll, ready};
use std::time::Duration;
use futures::future::BoxFuture;
use futures::{FutureExt};
use tower::discover::{Change, Discover};
use tower::Service;
use anyhow::anyhow;
use dashmap::DashMap;
use itertools::Itertools;
use crate::cursor_client::CursorClient;
use crate::discover::CursorClientDiscover;

pub(crate) trait Routable {
    fn route(&self) -> Route;
}

pub(crate) struct Router {
    discover: CursorClientDiscover,
    services: DashMap<String, CursorClient>
}

impl Router {
    pub(crate) fn new(discover: CursorClientDiscover) -> Self {
        metrics::describe_counter!("ton_router_miss_count", "Count of misses in router");
        metrics::describe_counter!("ton_router_delayed_miss_count", "Count of misses in router");
        metrics::describe_counter!("ton_router_delayed_count", "Count of delayed requests in router");

        Router {
            discover,
            services: Default::default()
        }
    }

    fn update_pending_from_discover(&mut self, cx: &mut Context<'_>, ) -> Poll<Option<Result<(), anyhow::Error>>> {
        loop {
            match ready!(Pin::new(&mut self.discover).poll_discover(cx)).transpose()? {
                None => return Poll::Ready(None),
                Some(Change::Remove(key)) => { self.services.remove(&key); }
                Some(Change::Insert(key, svc)) => { self.services.insert(key, svc); }
            }
        }
    }

    fn distance_to(&self, chain: &i32, criteria: &BlockCriteria) -> Option<i32> {
        self.services
            .iter()
            .filter_map(|s| s.contains(chain, criteria))
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

                return async move {
                    // TODO[akostylev0]
                    tokio::time::sleep(Duration::from_secs(2)).await;

                    let services = req.choose(&svcs);
                    if services.is_empty() {
                        metrics::increment_counter!("ton_router_delayed_miss_count");

                        Err(anyhow!("no services available for {:?}", req))
                    } else {
                        Ok(services)
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
            Route::Block { chain, criteria} => {
                services
                    .iter()
                    .filter(|s| s.contains(chain, criteria).is_some_and(|b| b <= 1))
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
