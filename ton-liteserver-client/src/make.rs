use crate::client::LiteServerClient;
use adnl_tcp::client::ServerKey;
use futures::future::BoxFuture;
use futures::FutureExt;
use std::future::Future;
use std::net::SocketAddrV4;
use std::pin::Pin;
use std::task::{Context, Poll};
use std::time::Duration;
use tokio::time::{Sleep, sleep};
use tokio_retry::strategy::{jitter, ExponentialBackoff};
use tower::Service;

pub struct MakeClient {
    addr: SocketAddrV4,
    key: ServerKey,
    retry_strategy: Box<dyn Iterator<Item = Duration> + Send>,
    sleep: Option<Pin<Box<Sleep>>>,
}

impl MakeClient {
    pub fn new(addr: SocketAddrV4, key: ServerKey) -> Self {
        let retry_strategy = Box::new(
            ExponentialBackoff::from_millis(100)
                .max_delay(Duration::from_secs(60))
                .map(jitter),
        );

        Self {
            addr,
            key,
            retry_strategy,
            sleep: None,
        }
    }
}

impl Service<()> for MakeClient {
    type Response = LiteServerClient;
    type Error = anyhow::Error;
    type Future = BoxFuture<'static, Result<Self::Response, Self::Error>>;

    fn poll_ready(&mut self, cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        match self.sleep.as_mut() {
            None => Poll::Ready(Ok(())),
            Some(sleep) => match sleep.as_mut().poll(cx) {
                Poll::Ready(()) => Poll::Ready(Ok(())),
                Poll::Pending => Poll::Pending,
            },
        }
    }

    fn call(&mut self, _: ()) -> Self::Future {
        if let Some(duration) = self.retry_strategy.as_mut().next() {
            self.sleep.replace(sleep(duration).boxed());
        }

        LiteServerClient::connect(self.addr, self.key).boxed()
    }
}
