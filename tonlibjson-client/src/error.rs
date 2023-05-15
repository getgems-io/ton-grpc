use std::task::{Context, Poll};
use anyhow::anyhow;
use derive_new::new;
use futures::future::MapErr;
use futures::TryFutureExt;
use tower::{Layer, Service};

#[derive(Default)]
pub struct ErrorLayer;

impl<S> Layer<S> for ErrorLayer {
    type Service = ErrorService<S>;

    fn layer(&self, inner: S) -> Self::Service {
        ErrorService::new(inner)
    }
}

#[derive(new, Clone)]
pub struct ErrorService<S> { inner: S }

impl<S, Req> Service<Req> for ErrorService<S> where
    S : Service<Req, Error = tower::BoxError>
{
    type Response = S::Response;
    type Error = anyhow::Error;
    type Future = MapErr<S::Future, fn(S::Error) -> Self::Error>;

    fn poll_ready(&mut self, cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        self.inner.poll_ready(cx).map_err(|e| anyhow!(e))
    }

    fn call(&mut self, req: Req) -> Self::Future {
        self.inner.call(req).map_err(|e| anyhow!(e))
    }
}
