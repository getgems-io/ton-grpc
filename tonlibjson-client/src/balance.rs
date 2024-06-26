use crate::block::{BlocksGetMasterchainInfo, BlocksMasterchainInfo};
use crate::cursor_client::{CursorClient, InnerClient};
use crate::error::Error;
use crate::request::{Callable, Specialized};
use derive_new::new;
use futures::{FutureExt, TryFutureExt};
use std::future::Future;
use std::hash::Hash;
use std::{
    pin::Pin,
    task::{Context, Poll},
};
use ton_client_util::router::route::ToRoute;
use ton_client_util::router::Router;
use tower::discover::Discover;
use tower::{MakeService, Service, ServiceExt};

#[derive(new)]
pub(crate) struct Balance<D>
where
    D: Discover<Service = CursorClient, Error = anyhow::Error> + Unpin,
    D::Key: Eq + Hash,
{
    router: Router<CursorClient, D>,
}

impl<R, D> Service<R> for Balance<D>
where
    R: ToRoute + Callable<InnerClient>,
    D: Discover<Service = CursorClient, Error = anyhow::Error> + Unpin,
    D::Key: Eq + Hash,
{
    type Response = R::Response;
    type Error = Error;
    type Future = Pin<Box<dyn Future<Output = Result<Self::Response, Self::Error>> + Send>>;

    fn poll_ready(&mut self, cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        <Router<CursorClient, D> as Service<&R>>::poll_ready(&mut self.router, cx)
            .map_err(Into::into)
    }

    fn call(&mut self, req: R) -> Self::Future {
        self.router
            .make_service(&req)
            .map_err(Into::into)
            .and_then(|svc| svc.oneshot(req))
            .map_err(Into::into)
            .boxed()
    }
}

impl<D> Service<Specialized<BlocksGetMasterchainInfo>> for Balance<D>
where
    D: Discover<Service = CursorClient, Error = anyhow::Error> + Unpin,
    D::Key: Eq + Hash,
{
    type Response = BlocksMasterchainInfo;
    type Error = Error;
    type Future = Pin<Box<dyn Future<Output = Result<Self::Response, Self::Error>> + Send>>;

    fn poll_ready(&mut self, cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        <Router<CursorClient, D> as Service<&Specialized<BlocksGetMasterchainInfo>>>::poll_ready(
            &mut self.router,
            cx,
        )
        .map_err(Into::into)
    }

    fn call(&mut self, req: Specialized<BlocksGetMasterchainInfo>) -> Self::Future {
        self.router
            .make_service(&req)
            .map_err(Into::into)
            .and_then(|svc| svc.oneshot(req))
            .map_err(Into::into)
            .boxed()
    }
}
