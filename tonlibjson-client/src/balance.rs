use std::{pin::Pin, task::{Context, Poll}};
use std::future::Future;
use futures::{TryFutureExt, FutureExt};
use derive_new::new;
use tower::{Service, ServiceExt};
use tower::discover::ServiceList;
use crate::block::{BlockIdExt, BlocksGetShards, BlocksLookupBlock, BlocksShards, GetMasterchainInfo, MasterchainInfo};
use crate::cursor_client::InnerClient;
use crate::error::ErrorService;
use crate::request::{Routable, Callable, Specialized};
use crate::router::Router;

#[derive(new)]
pub(crate) struct Balance { router: Router }

impl<R> Service<R> for Balance where R: Routable + Callable<InnerClient> {
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
