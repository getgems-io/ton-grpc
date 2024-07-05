use std::fmt::Debug;
use crate::request::Requestable;
use crate::tracked_client::TrackedClient;
use futures::{FutureExt, TryFutureExt};
use std::future::Future;
use std::hash::Hash;
use std::pin::Pin;
use std::task::{Context, Poll};
use ton_client_util::router::route::ToRoute;
use ton_client_util::router::Router;
use tower::discover::Discover;
use tower::{MakeService, Service, ServiceExt};

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error(transparent)]
    TrackedClient(#[from] crate::client::Error),
    #[error(transparent)]
    Router(#[from] ton_client_util::router::Error),
}

pub struct Balance<D, E>
where
    D: Discover<Service = TrackedClient, Error = E> + Unpin,
    D::Key: Eq + Hash,
{
    router: Router<TrackedClient, D>,
}

impl<D, E> Balance<D, E>
where
    D: Discover<Service = TrackedClient, Error = E> + Unpin,
    D::Key: Eq + Hash
{
    pub fn new(discover: D) -> Self {
        Self {
            router: Router::new(discover),
        }
    }
}

impl<R, D, E> Service<R> for Balance<D, E>
where
    R: ToRoute + Requestable + 'static,
    D: Discover<Service = TrackedClient, Error = E> + Unpin,
    D::Key: Eq + Hash,
    E: Into<Error> + Into<tower::BoxError> + Debug
{
    type Response = R::Response;
    type Error = Error;
    type Future = Pin<Box<dyn Future<Output = Result<Self::Response, Self::Error>> + Send>>;

    fn poll_ready(&mut self, cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        <Router<TrackedClient, D> as Service<&R>>::poll_ready(&mut self.router, cx)
            .map_err(Into::into)
    }

    fn call(&mut self, req: R) -> Self::Future {
        self.router
            .make_service(&req)
            .and_then(|svc| svc.oneshot(req).map_err(Into::into))
            .map_err(Into::into)
            .boxed()
    }
}
