use crate::client::LiteServerClient;
use adnl_tcp::client::ServerKey;
use futures::future::BoxFuture;
use futures::FutureExt;
use std::net::SocketAddrV4;
use std::task::{Context, Poll};
use tower::Service;

#[derive(Default)]
pub struct MakeClient;

impl Service<(SocketAddrV4, ServerKey)> for MakeClient {
    type Response = LiteServerClient;
    type Error = anyhow::Error;
    type Future = BoxFuture<'static, Result<Self::Response, Self::Error>>;

    fn poll_ready(&mut self, _: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        Poll::Ready(Ok(()))
    }

    fn call(&mut self, req: (SocketAddrV4, ServerKey)) -> Self::Future {
        LiteServerClient::connect(req.0, req.1).boxed()
    }
}
