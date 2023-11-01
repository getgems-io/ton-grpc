use std::{pin::Pin, task::{Context, Poll}};
use std::future::Future;
use std::sync::Arc;
use std::time::Duration;
use futures::{ready, TryFutureExt, FutureExt};
use tower::{Service, ServiceExt};
use tower::discover::{Change, Discover, ServiceList};
use anyhow::anyhow;
use dashmap::DashMap;
use derive_new::new;
use futures::future::BoxFuture;
use itertools::Itertools;
use tokio_retry::Retry;
use tokio_retry::strategy::{FibonacciBackoff, jitter};
use crate::block::{BlockIdExt, BlocksGetShards, BlocksLookupBlock, BlocksShards, GetMasterchainInfo, MasterchainInfo};
use crate::cursor_client::{CursorClient, InnerClient};
use crate::discover::CursorClientDiscover;
use crate::error::ErrorService;
use crate::request::{Routable, Callable, Specialized};

#[derive(Debug, Clone, Copy)]
pub enum BlockCriteria {
    Seqno { shard: i64, seqno: i32 },
    LogicalTime(i64)
}

#[derive(Debug, Clone, Copy)]
pub enum Route {
    Any,
    Block { chain: i32, criteria: BlockCriteria },
    Latest
}

impl Route {
    pub fn choose<T : Iterator<Item=CursorClient>>(&self, services: T) -> Vec<CursorClient> {
        match self {
            Route::Any => { services.collect() },
            Route::Block { chain, criteria} => {
                services
                    .filter(|s| s.contains(chain, criteria))
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

                idxs.into_iter().map(|(s, _)| s).collect()
            }
        }
    }
}

pub struct Router {
    discover: CursorClientDiscover,
    services: Arc<DashMap<String, CursorClient>>
}

impl Router {
    pub fn new(discover: CursorClientDiscover) -> Self {
        Router {
            discover,
            services: Default::default()
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

#[derive(new)]
pub struct Balance { router: Router }

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
        let req = *req;
        let services = Arc::clone(&self.services);

        let retry = FibonacciBackoff::from_millis(512)
            .max_delay(Duration::from_millis(4096))
            .map(jitter)
            .take(16);

        Retry::spawn(retry, move || {
            let svc = req.choose(services.iter().map(|s| s.clone()));

            std::future::ready(if svc.is_empty() {
                Err(anyhow!("no service available"))
            } else {
                Ok(svc)
            })
        }).boxed()
    }
}

impl<R> Service<R> for Balance where R: Routable + Callable<InnerClient> + Clone {
    type Response = R::Response;
    type Error = anyhow::Error;
    type Future = Pin<Box<dyn Future<Output = Result<Self::Response, Self::Error>> + Send>>;

    fn poll_ready(&mut self, cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        self.router.poll_ready(cx)
    }

    fn call(&mut self, req: R) -> Self::Future {
        self.router
            .call(&req.route())
            .and_then(|svc| ErrorService::new(tower::balance::p2c::Balance::new(ServiceList::new::<R>(svc)))
                .oneshot(req))
            .boxed()
    }
}

impl Service<Specialized<GetMasterchainInfo>> for Balance {
    type Response = MasterchainInfo;
    type Error = anyhow::Error;
    type Future = Pin<Box<dyn Future<Output = Result<Self::Response, Self::Error>> + Send>>;

    fn poll_ready(&mut self, cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        self.router.poll_ready(cx)
    }

    fn call(&mut self, req: Specialized<GetMasterchainInfo>) -> Self::Future {
        self.router
            .call(&req.route())
            .and_then(|svc| ErrorService::new(tower::balance::p2c::Balance::new(
                ServiceList::new::<Specialized<GetMasterchainInfo>>(svc))).oneshot(req))
            .boxed()
    }
}

// TODO[akostylev0] generics
impl Service<Specialized<BlocksGetShards>> for Balance {
    type Response = BlocksShards;
    type Error = anyhow::Error;
    type Future = Pin<Box<dyn Future<Output = Result<Self::Response, Self::Error>> + Send>>;

    fn poll_ready(&mut self, cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        self.router.poll_ready(cx)
    }

    fn call(&mut self, req: Specialized<BlocksGetShards>) -> Self::Future {
        self.router
            .call(&req.route())
            .and_then(|svc| ErrorService::new(tower::balance::p2c::Balance::new(
                ServiceList::new::<Specialized<BlocksGetShards>>(svc))).oneshot(req))
            .boxed()
    }
}

// TODO[akostylev0] generics
impl Service<Specialized<BlocksLookupBlock>> for Balance {
    type Response = BlockIdExt;
    type Error = anyhow::Error;
    type Future = Pin<Box<dyn Future<Output = Result<Self::Response, Self::Error>> + Send>>;

    fn poll_ready(&mut self, cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        self.router.poll_ready(cx)
    }

    fn call(&mut self, req: Specialized<BlocksLookupBlock>) -> Self::Future {
        self.router
            .call(&req.route())
            .and_then(|svc| ErrorService::new(tower::balance::p2c::Balance::new(
                ServiceList::new::<Specialized<BlocksLookupBlock>>(svc))).oneshot(req))
            .boxed()
    }
}
