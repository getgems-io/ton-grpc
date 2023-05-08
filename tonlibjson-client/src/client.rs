use std::future::Future;
use std::pin::Pin;
use std::task::{Context, Poll};
use std::time::Duration;
use anyhow::anyhow;
use serde_json::Value;
use futures::{Sink, Stream};
use tokio_tower::multiplex::TagStore;
use tower::Service;
use crate::request::{DataOrError, Request, RequestId, Response};
use futures::FutureExt;

#[derive(Default)]
struct Transport { inner: tonlibjson_sys::Client }

impl Sink<Request> for Transport {
    type Error = anyhow::Error;

    fn poll_ready(self: Pin<&mut Self>, _: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        Poll::Ready(Ok(()))
    }

    fn start_send(self: Pin<&mut Self>, item: Request) -> Result<(), Self::Error> {
        let req = serde_json::to_string(&item)?;

        self.inner.send(&req)
    }

    fn poll_flush(self: Pin<&mut Self>, _: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        Poll::Ready(Ok(()))
    }

    fn poll_close(self: Pin<&mut Self>, _: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        Poll::Ready(Ok(()))
    }
}

impl Stream for Transport {
    type Item = anyhow::Result<Response>;

    fn poll_next(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        let response = self.inner.receive(Duration::from_secs(0))
            .and_then(|s| serde_json::from_str::<Response>(s).map_err(Into::into));

        match response {
            Ok(response) => {
                return Poll::Ready(Some(Ok(response)))
            }
            Err(_) => {
                cx.waker().wake_by_ref();

                Poll::Pending
            }
        }
    }
}

impl TagStore<Request, Response> for Transport {
    type Tag = RequestId;

    fn assign_tag(self: Pin<&mut Self>, r: &mut Request) -> Self::Tag { r.id }

    fn finish_tag(self: Pin<&mut Self>, r: &Response) -> Self::Tag { r.id }
}

pub struct Client {
    inner: tokio_tower::multiplex::Client<Transport, tokio_tower::Error<Transport, Request>, Request>
}

impl Client {
    pub fn new() -> Self {
        Self { inner: tokio_tower::multiplex::Client::new(Transport::default()) }
    }
}

impl Service<Request> for Client {
    type Response = Value;
    type Error = anyhow::Error;
    type Future = Pin<Box<dyn Future<Output = Result<Self::Response, Self::Error>> + Send>>;

    fn poll_ready(&mut self, cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        self.inner.poll_ready(cx).map_err(|e| anyhow!(e))
    }

    fn call(&mut self, req: Request) -> Self::Future {
        self.inner.call(req)
            .map(|response| {
                match response {
                    Ok(response) => match response.body {
                        DataOrError::Error(e) => { Err(anyhow!(e)) }
                        DataOrError::Data(data) => { Ok(data) }
                    },
                    Err(e) => Err(anyhow!(e))
                }
            })
            .boxed()
    }
}
