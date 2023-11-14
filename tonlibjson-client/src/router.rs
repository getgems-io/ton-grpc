use std::collections::HashMap;
use std::future::Ready;
use std::pin::Pin;
use std::task::{Context, Poll, ready};
use anyhow::anyhow;
use itertools::Itertools;
use tower::discover::{Change, Discover};
use tower::Service;
use crate::cursor_client::CursorClient;
use crate::discover::CursorClientDiscover;

pub(crate) trait Routable {
    fn route(&self) -> Route;
}

pub(crate) struct Router {
    discover: CursorClientDiscover,
    services: HashMap<String, CursorClient>
}

impl Router {
    pub(crate) fn new(discover: CursorClientDiscover) -> Self {
        metrics::describe_counter!("ton_router_miss_count", "Count of misses in router");

        Router {
            discover,
            services: HashMap::new()
        }
    }

    fn update_pending_from_discover(
        &mut self,
        cx: &mut Context<'_>,
    ) -> Poll<Option<Result<(), anyhow::Error>>> {
        loop {
            match ready!(Pin::new(&mut self.discover).poll_discover(cx))
                .transpose()
                .map_err(|e| anyhow!(e))?
            {
                None => return Poll::Ready(None),
                Some(Change::Remove(key)) => { self.services.remove(&key); }
                Some(Change::Insert(key, svc)) => { self.services.insert(key, svc); }
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
    type Future = Ready<Result<Self::Response, Self::Error>>;

    fn poll_ready(&mut self, cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        let _ = self.update_pending_from_discover(cx)?;

        if self.services.values().any(|s| s.edges_defined()) {
            Poll::Ready(Ok(()))
        } else {
            cx.waker().wake_by_ref();

            Poll::Pending
        }
    }

    fn call(&mut self, req: &Route) -> Self::Future {
        let services = req.choose(self.services.values());

        let response = if services.is_empty() {
            metrics::increment_counter!("ton_router_miss_count");

            Err(anyhow!("no services available for {:?}", req))
        } else {
            Ok(services)
        };

        std::future::ready(response)
    }
}

impl Route {
    pub(crate) fn choose<'a, T : Iterator<Item=&'a CursorClient>>(&self, services: T) -> Vec<CursorClient> {
        match self {
            Route::Block { chain, criteria} => {
                services
                    .filter(|s| s.contains(chain, criteria).is_some_and(|b| b <= 1))
                    .cloned()
                    .collect()
            },
            Route::Latest => {
                let groups = services
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

                idxs.into_iter().map(|(s, _)| s).cloned().collect()
            }
        }
    }
}
