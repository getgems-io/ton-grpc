use crate::router::route::ToRoute;
use crate::router::{Routed, Router};
use futures::FutureExt;
use futures::TryFutureExt;
use std::fmt::Debug;
use std::future::Future;
use std::hash::Hash;
use std::pin::Pin;
use std::task::{Context, Poll};
use tower::discover::Discover;
use tower::load::Load;
use tower::{MakeService, Service, ServiceExt};

pub struct Balance<S, D>
where
    D: Discover<Service = S>,
    D::Key: Hash,
{
    router: Router<S, D>,
}

impl<S, D> Balance<S, D>
where
    D: Discover<Service = S, Error: Debug> + Unpin,
    D::Key: Hash,
{
    pub fn new(discover: D) -> Self {
        let router = Router::new(discover);

        Balance { router }
    }
}

impl<S, R, D> Service<R> for Balance<S, D>
where
    R: ToRoute + Sync + Send + 'static,
    S: Clone
        + Service<R, Error: Into<tower::BoxError>, Future: Send>
        + Load
        + Routed
        + Send
        + 'static,
    D: Discover<Service = S, Error: Into<tower::BoxError> + Debug> + Unpin + Send,
    D::Key: Eq + Hash + Clone + Send,
    S::Metric: Debug,
{
    type Response = S::Response;
    type Error = tower::BoxError;
    type Future = Pin<Box<dyn Future<Output = Result<Self::Response, Self::Error>> + Send>>;

    fn poll_ready(&mut self, cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        MakeService::poll_ready(&mut self.router, cx).map_err(Into::into)
    }

    fn call(&mut self, req: R) -> Self::Future {
        self.router
            .make_service(&req)
            .and_then(|svc| svc.oneshot(req))
            .boxed()
    }
}
