use std::task::{Context, Poll};
use tower::load::Load;
use tower::Service;

pub struct LoadRef<'a, L> {
    inner: &'a mut L,
}

impl<'a, L> LoadRef<'a, L> {
    pub fn new(inner: &'a mut L) -> Self {
        Self { inner }
    }
}

impl<'a, L> Load for LoadRef<'a, L>
where
    L: Load,
{
    type Metric = L::Metric;

    fn load(&self) -> Self::Metric {
        self.inner.load()
    }
}

impl<'a, L, R> Service<R> for LoadRef<'a, L> where L: Service<R> {
    type Response = L::Response;
    type Error = L::Error;
    type Future = L::Future;

    fn poll_ready(&mut self, cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        self.inner.poll_ready(cx)
    }

    fn call(&mut self, req: R) -> Self::Future {
        self.inner.call(req)
    }
}
