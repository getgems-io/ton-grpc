use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;
use std::task::{Context, Poll};
use tower::Service;
use pin_project::pin_project;
use tower::load::Load;

type Counter = Arc<std::sync::atomic::AtomicU32>;

#[pin_project]
pub struct ResponseFuture<T> {
    #[pin]
    inner: T,
    counter: Counter
}

impl<T> ResponseFuture<T> {
    pub fn new(inner: T, counter: Counter) -> ResponseFuture<T> {
        counter.fetch_sub(1, std::sync::atomic::Ordering::Relaxed);

        Self { inner, counter }
    }

    pub fn get_ref(&self) -> &T {
        &self.inner
    }

    pub fn get_mut(&mut self) -> &mut T {
        &mut self.inner
    }
    pub fn into_inner(self) -> T {
        self.inner
    }
}

impl<F, T, E> Future for ResponseFuture<F>
    where
        F: Future<Output = Result<T, E>>,
{
    type Output = Result<T, E>;

    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        let this = self.project();

        match this.inner.poll(cx) {
            Poll::Ready(t) => {
                this.counter.fetch_sub(1, std::sync::atomic::Ordering::Relaxed);

                Poll::Ready(t)
            }
            Poll::Pending => { Poll::Pending }
        }
    }
}


struct ConcurrencyMetric<S> {
    inner: S,
    counter: Counter
}

impl<S> ConcurrencyMetric<S> {
    pub fn new(inner: S) -> Self {
        Self { inner, counter: Counter::default() }
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

        ResponseFuture::new(future, Arc::clone(&self.counter))
    }
}

impl<T> Load for ConcurrencyMetric<T> {
    type Metric = u32;

    fn load(&self) -> Self::Metric {
        self.counter.load(std::sync::atomic::Ordering::Relaxed)
    }
}
