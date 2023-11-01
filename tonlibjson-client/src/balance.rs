use std::{pin::Pin, task::{Context, Poll}};
use std::collections::HashMap;
use std::future::{Future, Ready};
use futures::{ready, TryFutureExt, FutureExt};
use tower::{Service, ServiceExt};
use tower::discover::{Change, Discover, ServiceList};
use anyhow::anyhow;
use derive_new::new;
use itertools::Itertools;
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
    pub fn choose<'a, T : Iterator<Item=&'a CursorClient>>(&self, services: T) -> Vec<CursorClient> {
        match self {
            Route::Any => { services.cloned().collect() },
            Route::Block { chain, criteria} => {
                services
                    .filter(|s| s.contains(chain, criteria))
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

pub struct Router {
    discover: CursorClientDiscover,
    services: HashMap<String, CursorClient>
}

impl Router {
    pub fn new(discover: CursorClientDiscover) -> Self {
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

#[derive(new)]
pub struct Balance { router: Router }

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
            Err(anyhow!("no services available for {:?}", req))
        } else {
            Ok(services)
        };

        std::future::ready(response)
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
