use crate::request::Requestable;
use futures::{FutureExt, TryFutureExt};
use std::fmt::Debug;
use std::future::Future;
use std::hash::Hash;
use std::pin::Pin;
use std::task::{Context, Poll};
use ton_client_util::router::route::ToRoute;
use ton_client_util::router::{Routed, Router};
use tower::discover::Discover;
use tower::load::Load;
use tower::{MakeService, Service, ServiceExt};

pub struct Balance<S, D, E>
where
    D: Discover<Service = S, Error = E>,
    D::Key: Eq + Hash,
{
    router: Router<S, D>,
}

impl<S, D, E> Balance<S, D, E>
where
    D: Discover<Service = S, Error = E>,
    D::Key: Eq + Hash,
{
    pub fn new(discover: D) -> Self {
        Self {
            router: Router::new(discover),
        }
    }
}

impl<S, R, D, SE, DE> Service<R> for Balance<S, D, DE>
where
    R: ToRoute + Requestable + 'static,
    D: Discover<Service = S, Error = DE> + Unpin,
    D::Key: Eq + Hash,
    DE: Into<tower::BoxError> + Debug,
    S: Clone + Routed + Debug + Load + Send + 'static,
    S: Service<R, Response = R::Response, Error = SE>,
    S::Future: Send,
    <S as Load>::Metric: Debug,
    SE: Into<tower::BoxError> + Debug + 'static,
{
    type Response = R::Response;
    type Error = tower::BoxError;
    type Future = Pin<Box<dyn Future<Output = Result<Self::Response, Self::Error>>>>;

    fn poll_ready(&mut self, cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        MakeService::poll_ready(&mut self.router, cx).map_err(Into::into)
    }

    fn call(&mut self, req: R) -> Self::Future {
        self.router
            .make_service(&req)
            .and_then(|svc| svc.oneshot(req).map_err(Into::into))
            .map_err(Into::into)
            .boxed()
    }
}
