use std::task::{Context, Poll};
use anyhow::anyhow;
use derive_new::new;
use futures::future::MapErr;
use futures::TryFutureExt;
use thiserror::Error;
use tower::{Layer, Service};
use ton_client_utils::router::RouterError;

#[derive(Debug, Error)]
pub enum Error {
    #[error(transparent)]
    Router(#[from] RouterError),
    #[error(transparent)]
    Custom(#[from] anyhow::Error)
}

impl From<tower::BoxError> for Error {
    fn from(err: tower::BoxError) -> Self {
        Self::Custom(anyhow!(err))
    }
}

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

impl<S, Req, E: Into<Error>> Service<Req> for ErrorService<S> where
    S : Service<Req, Error=E>
{
    type Response = S::Response;
    type Error = anyhow::Error;
    type Future = MapErr<S::Future, fn(S::Error) -> Self::Error>;

    fn poll_ready(&mut self, cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        self.inner.poll_ready(cx).map_err(|e| e.into().into())
    }

    fn call(&mut self, req: Req) -> Self::Future {
        self.inner.call(req).map_err(|e| e.into().into())
    }
}
