use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;
use std::task::{Context, Poll};
use tower::Service;
use pin_project::{pin_project, pinned_drop};
use tower::load::Load;

type Counter = Arc<std::sync::atomic::AtomicI32>;

#[pin_project(PinnedDrop)]
pub struct ResponseFuture<T> {
    #[pin]
    inner: T,
    inflight: Counter
}

impl<T> ResponseFuture<T> {
    pub fn new(inner: T, inflight: Counter) -> ResponseFuture<T> {
        inflight.fetch_add(1, std::sync::atomic::Ordering::Relaxed);

        Self { inner, inflight  }
    }
}

#[pinned_drop]
impl<T> PinnedDrop for ResponseFuture<T> {
    fn drop(self: Pin<&mut Self>) {
        self.inflight.fetch_sub(1, std::sync::atomic::Ordering::Relaxed);
    }
}

impl<F, T, E> Future for ResponseFuture<F>
    where
        F: Future<Output = Result<T, E>>,
{
    type Output = Result<T, E>;

    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        self.project().inner.poll(cx)
    }
}


#[derive(Clone, Debug)]
pub struct ConcurrencyMetric<S> {
    inner: S,
    inflight: Counter
}

impl<S> ConcurrencyMetric<S> {
    pub fn new(inner: S) -> Self {
        Self { inner, inflight: Counter::default() }
    }

    pub fn get_ref(&self) -> &S {
        &self.inner
    }

    pub fn get_mut(&mut self) -> &mut S {
        &mut self.inner
    }
    pub fn into_inner(self) -> S {
        self.inner
    }
}

impl<S, Request> Service<Request> for ConcurrencyMetric<S>
    where
        S: Service<Request> {
    type Response = S::Response;
    type Error = S::Error;
    type Future = ResponseFuture<S::Future>;

    fn poll_ready(&mut self, cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        self.inner.poll_ready(cx)
    }

    fn call(&mut self, req: Request) -> Self::Future {
        let future = self.inner.call(req);

        ResponseFuture::new(future, Arc::clone(&self.inflight))
    }
}

impl<T> Load for ConcurrencyMetric<T> {
    type Metric = i32;

    fn load(&self) -> Self::Metric {
        self.inflight.load(std::sync::atomic::Ordering::Relaxed)
    }
}
