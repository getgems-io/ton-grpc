use pin_project::pin_project;
use std::future::Future;
use std::pin::Pin;
use std::task::{Context, Poll};
use std::time::Duration;
use tokio::time::{sleep, Sleep};
use tower::load::Load;
use tower::timeout::error::Elapsed;
use tower::{BoxError, Layer, Service};

pub struct TimeoutLayer {
    default_timeout: Duration,
}

impl TimeoutLayer {
    pub fn new(default_timeout: Duration) -> Self {
        Self { default_timeout }
    }
}

impl<S> Layer<S> for TimeoutLayer {
    type Service = Timeout<S>;

    fn layer(&self, inner: S) -> Self::Service {
        Timeout::new(inner, self.default_timeout)
    }
}

pub trait ToTimeout {
    fn to_timeout(&self) -> Option<Duration> {
        None
    }
}

#[derive(Debug)]
pub struct Timeout<T> {
    inner: T,
    default_timeout: Duration,
}

impl<T> Clone for Timeout<T>
where
    T: Clone,
{
    fn clone(&self) -> Self {
        Self {
            inner: self.inner.clone(),
            default_timeout: self.default_timeout.clone(),
        }
    }
}

impl<T> Timeout<T> {
    pub fn new(inner: T, default_timeout: Duration) -> Self {
        Self {
            inner,
            default_timeout,
        }
    }
}

impl<S, Request> Service<Request> for Timeout<S>
where
    Request: ToTimeout,
    S: Service<Request>,
    S::Error: Into<BoxError>,
{
    type Response = S::Response;
    type Error = BoxError;
    type Future = ResponseFuture<S::Future>;

    fn poll_ready(&mut self, cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        match self.inner.poll_ready(cx) {
            Poll::Pending => Poll::Pending,
            Poll::Ready(r) => Poll::Ready(r.map_err(Into::into)),
        }
    }

    fn call(&mut self, request: Request) -> Self::Future {
        let timeout = request.to_timeout().unwrap_or(self.default_timeout);
        let response = self.inner.call(request);
        let sleep = sleep(timeout);

        ResponseFuture::new(response, sleep)
    }
}

impl<T> Load for Timeout<T> where T: Load {
    type Metric = T::Metric;

    fn load(&self) -> Self::Metric {
        self.inner.load()
    }
}

#[derive(Debug)]
#[pin_project]
pub struct ResponseFuture<T> {
    #[pin]
    response: T,
    #[pin]
    sleep: Sleep,
}

impl<T> ResponseFuture<T> {
    pub(crate) fn new(response: T, sleep: Sleep) -> Self {
        ResponseFuture { response, sleep }
    }
}

impl<F, T, E> Future for ResponseFuture<F>
where
    F: Future<Output = Result<T, E>>,
    E: Into<BoxError>,
{
    type Output = Result<T, BoxError>;

    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        let this = self.project();

        match this.response.poll(cx) {
            Poll::Ready(v) => return Poll::Ready(v.map_err(Into::into)),
            Poll::Pending => {}
        }

        match this.sleep.poll(cx) {
            Poll::Pending => Poll::Pending,
            Poll::Ready(_) => Poll::Ready(Err(Elapsed::new().into())),
        }
    }
}
