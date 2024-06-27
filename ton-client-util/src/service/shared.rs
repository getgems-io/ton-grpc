use pin_project::pin_project;
use std::future::Future;
use std::pin::Pin;
use std::sync::{Arc, Mutex};
use std::task::{ready, Context, Poll};
use tower::load::Load;
use tower::{Layer, Service};

#[derive(Default)]
pub struct SharedLayer;

impl<S> Layer<S> for SharedLayer {
    type Service = SharedService<S>;

    fn layer(&self, inner: S) -> Self::Service {
        SharedService::new(inner)
    }
}

#[derive(Debug)]
pub struct SharedService<S> {
    inner: Arc<Mutex<S>>,
}

impl<S> Clone for SharedService<S> {
    fn clone(&self) -> Self {
        Self {
            inner: Arc::clone(&self.inner),
        }
    }
}

impl<S> SharedService<S> {
    pub fn new(inner: S) -> Self {
        Self {
            inner: Arc::new(Mutex::new(inner)),
        }
    }
}

impl<S, Req> Service<Req> for SharedService<S>
where
    S: Service<Req>,
{
    type Response = S::Response;
    type Error = S::Error;
    type Future = ResponseFuture<S, Req>;

    fn poll_ready(&mut self, cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        match self.inner.try_lock() {
            Ok(mut lock) => lock.poll_ready(cx),
            Err(_) => {
                cx.waker().wake_by_ref();

                Poll::Pending
            }
        }
    }

    fn call(&mut self, req: Req) -> Self::Future {
        ResponseFuture::new(Arc::clone(&self.inner), req)
    }
}

impl<S> Load for SharedService<S>
where
    S: Load,
{
    type Metric = S::Metric;

    fn load(&self) -> Self::Metric {
        self.inner.lock().unwrap().load()
    }
}

#[pin_project]
pub struct ResponseFuture<S: Service<Req>, Req> {
    request: Option<Req>,
    #[pin]
    state: ResponseState<S, Req>,
}

#[pin_project(project = ResponseStateProj)]
enum ResponseState<S, Req>
where
    S: Service<Req>,
{
    Locking {
        service: Arc<Mutex<S>>,
    },
    Waiting {
        #[pin]
        future: S::Future,
    },
}

impl<S, Req> ResponseFuture<S, Req>
where
    S: Service<Req>,
{
    pub fn new(service: Arc<Mutex<S>>, request: Req) -> Self {
        Self {
            request: Some(request),
            state: ResponseState::Locking { service },
        }
    }
}

impl<S, Req> Future for ResponseFuture<S, Req>
where
    S: Service<Req>,
{
    type Output = Result<S::Response, S::Error>;

    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        let mut this = self.project();

        loop {
            match this.state.as_mut().project() {
                ResponseStateProj::Locking { service } => {
                    let future = {
                        if let Ok(mut guard) = service.try_lock() {
                            let req = this
                                .request
                                .take()
                                .expect("Future was polled after completion");

                            guard.call(req)
                        } else {
                            cx.waker().wake_by_ref();

                            return Poll::Pending;
                        }
                    };

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
