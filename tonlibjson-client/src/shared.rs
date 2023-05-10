use std::future::Future;
use std::pin::Pin;
use std::sync::{Arc};
use std::task::{Context, Poll};
use futures::FutureExt;
use tokio::sync::RwLock;
use tower::{Layer, Service};
use tower::load::Load;

#[derive(Default)]
pub struct SharedLayer;

impl<S> Layer<S> for SharedLayer {
    type Service = SharedService<S>;

    fn layer(&self, inner: S) -> Self::Service {
        SharedService::new(inner)
    }
}

pub struct SharedService<S> {
    inner: Arc<RwLock<S>>
}

impl<S> Clone for SharedService<S> {
    fn clone(&self) -> Self {
        Self {
            inner: self.inner.clone()
        }
    }
}

impl<S> SharedService<S> {
    pub fn new(inner: S) -> Self {
        Self { inner: Arc::new(RwLock::new(inner)) }
    }
}

impl<S, Req> Service<Req> for SharedService<S>
    where S : Service<Req> + Sync + Send + 'static,
          S::Future : Send,
          Req: Send + 'static,
{
    type Response = S::Response;
    type Error = S::Error;
    type Future = Pin<Box<dyn Future<Output = Result<Self::Response, Self::Error>> + Send>>;

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
        let inner = self.inner.clone();

        async move {
            inner.write().await.call(req).await
        }.boxed()
    }
}

impl<S> Load for SharedService<S> where S : Load {
    type Metric = S::Metric;

    fn load(&self) -> Self::Metric {
        tokio::task::block_in_place(|| {
            let svc = self.inner.blocking_read();

            svc.load()
        })
    }
}
