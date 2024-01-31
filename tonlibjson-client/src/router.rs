use std::pin::Pin;
use std::task::{Context, Poll, ready};
use futures::future::BoxFuture;
use futures::{FutureExt, TryFutureExt};
use tower::discover::{Change, Discover};
use tower::Service;
use anyhow::anyhow;
use dashmap::DashMap;
use itertools::Itertools;
use tokio_retry::Retry;
use tokio_retry::strategy::{FibonacciBackoff, jitter};
use crate::cursor_client::CursorClient;
use crate::discover::CursorClientDiscover;

pub(crate) trait Routable {
    fn route(&self) -> Route { Route::Latest }
}

pub(crate) struct Router {
    discover: CursorClientDiscover,
    services: DashMap<String, CursorClient>
}

impl Router {
    pub(crate) fn new(discover: CursorClientDiscover) -> Self {
        metrics::describe_counter!("ton_router_miss_count", "Count of misses in router");
        metrics::describe_counter!("ton_router_fallback_hit_count", "Count of fallback request hits in router");
        metrics::describe_counter!("ton_router_delayed_count", "Count of delayed requests in router");
        metrics::describe_counter!("ton_router_delayed_hit_count", "Count of delayed request hits in router");
        metrics::describe_counter!("ton_router_delayed_miss_count", "Count of delayed request misses in router");

        Router { discover, services: Default::default() }
    }

    fn update_pending_from_discover(&mut self, cx: &mut Context<'_>, ) -> Poll<Option<Result<(), anyhow::Error>>> {
        loop {
            match ready!(Pin::new(&mut self.discover).poll_discover(cx)).transpose()? {
                None => return Poll::Ready(None),
                Some(Change::Remove(key)) => {
                    self.services.remove(&key);
                }
                Some(Change::Insert(key, svc)) => {
                    self.services.insert(key, svc);
                }
            }
        }
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
        match req.choose(&self.services) {
            Ok(services) => {
                return std::future::ready(Ok(services)).boxed()
            },
            Err(RouterError::RouteUnknown) => {
                metrics::counter!("ton_router_miss_count").increment(1);
                match Route::Latest.choose(&self.services) {
                    Ok(services) => { std::future::ready(Ok(services)).boxed() }
                    Err(_) => { std::future::ready(Err(anyhow!("no services available for {:?}", req))).boxed() }
                }
            },
            Err(RouterError::RouteNotAvailable) => {
                metrics::counter!("ton_router_delayed_count").increment(1);

                let req = *req;
                let svcs = self.services.clone();
                Retry::spawn(FibonacciBackoff::from_millis(32).map(jitter).take(10), move || {
                    let req = req.clone();
                    let svcs = svcs.clone();

                    async move { req.choose(&svcs) }
                })
                    .map_err(move |_| anyhow!("no services available for {:?}", req))
                    .boxed()
            }
        }
    }
}


#[derive(Debug, thiserror::Error)]
enum RouterError {
    #[error("route is not available at this moment")]
    RouteNotAvailable,
    #[error("route is unknown")]
    RouteUnknown
}

impl Route {
    fn choose(&self, services: &DashMap<String, CursorClient>) -> Result<Vec<CursorClient>, RouterError> {
        match self {
            Route::Block { chain, criteria } => {
                let clients: Vec<CursorClient> = services
                    .iter()
                    .filter(|s| s.contains(chain, criteria))
                    .map(|s| s.clone())
                    .collect();

                if clients.is_empty() {
                    if services.iter().any(|s| s.contains_not_available(chain, criteria)) {
                        Err(RouterError::RouteNotAvailable)
                    } else {
                        Err(RouterError::RouteUnknown)
                    }
                } else {
                    Ok(clients)
                }
            },
            Route::Latest => {
                let groups = services
                    .iter()
                    .filter_map(|s| s.last_seqno().map(|seqno| (s, seqno)))
                    .sorted_unstable_by_key(|(_, seqno)| -seqno)
                    .group_by(|(_, seqno)| *seqno);

                if let Some((_, group)) = (&groups).into_iter().next() {
                    return Ok(group
                        .into_iter()
                        .map(|(s, _)| s.clone())
                        .collect());
                }

                return Err(RouterError::RouteUnknown);
            }
        }
    }
}
