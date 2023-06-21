use std::future::Future;
use std::pin::Pin;
use std::sync::{Arc, RwLock};
use std::task::{Context, Poll};
use futures::ready;
use pin_project::pin_project;
use tower::{Layer, Service};
use tower::load::Load;
use crate::shared::ResponseState::Locking;

#[derive(Default)]
pub struct SharedLayer;

impl<S: Send> Layer<S> for SharedLayer {
    type Service = SharedService<S>;

    fn layer(&self, inner: S) -> Self::Service {
        SharedService::new(inner)
    }
}

pub struct SharedService<S: Send> {
    inner: Arc<RwLock<S>>
}

impl<S: Send> Clone for SharedService<S> {
    fn clone(&self) -> Self {
        Self {
            inner: self.inner.clone()
        }
    }
}

impl<S: Send> SharedService<S> {
    pub fn new(inner: S) -> Self {
        Self { inner: Arc::new(RwLock::new(inner)) }
    }
}

impl<S, Req> Service<Req> for SharedService<S>
    where S : Service<Req> + Send + 'static,
          S::Future : Send,
          S::Error: Send + Sync + 'static,
          S::Response: Send,
          Req: Send + 'static {
    type Response = S::Response;
    type Error = S::Error;
    type Future = ResponseFuture<S, Req>;

    fn poll_ready(&mut self, cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        match self.inner.try_write() {
            Ok(mut lock) => {
                lock.poll_ready(cx)
            }
            Err(_) => {
                cx.waker().wake_by_ref();

                Poll::Pending
            }
        }
    }

    fn call(&mut self, req: Req) -> Self::Future {
        ResponseFuture::new(&self.inner, req)
    }
}

impl<S> Load for SharedService<S> where S : Load + Send {
    type Metric = S::Metric;

    fn load(&self) -> Self::Metric {
        self.inner.read().unwrap().load()
    }
}

#[pin_project]
pub struct ResponseFuture<S: Service<Req>, Req>
{
    request: Option<Req>,
    #[pin]
    state: ResponseState<S, Req>
}

#[pin_project(project = ResponseStateProj)]
enum ResponseState<S, Req> where S : Service<Req> {
    Locking { service: Arc<RwLock<S>> },
    Waiting { #[pin] future: S::Future }
}

impl<S: Service<Req>, Req> ResponseFuture<S, Req> where
    S: Service<Req> + Send + 'static,
    S::Error: Send + Sync + 'static,
    S::Response: Send + 'static,
    S::Future: Send + 'static,
    Req: Send + 'static
{
    pub fn new(service: &Arc<RwLock<S>>, request: Req) -> Self {
        Self {
            request: Some(request),
            state: Locking { service: Arc::clone(service) }
        }
    }
}

impl<S: Service<Req>, Req> Future for ResponseFuture<S, Req> where
    S: Service<Req> + Send + 'static,
    S::Error: Send + Sync + 'static,
    S::Response: Send + 'static,
    S::Future: Send + 'static,
    Req: Send + 'static
{
    type Output = Result<S::Response, S::Error>;

    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        let mut this = self.project();

        loop {
            match this.state.as_mut().project() {
                ResponseStateProj::Locking { service } => {
                    let future;

                    {
                        if let Ok(mut guard) = service.try_write() {
                            let req = this.request.take()
                                .expect("Future was polled after completion");
                            future = guard.call(req);
                        } else {
                            cx.waker().wake_by_ref();

                            return Poll::Pending;
                        }
                    }

                    this.state.set(ResponseState::Waiting { future });
                }
                ResponseStateProj::Waiting { future } => {
                    let response = ready!(future.poll(cx));

                    return Poll::Ready(response);
                }
            }
        }
    }
}
