use crate::client::TonlibjsonClient;
use anyhow::Context as ErrorContext;
use futures::FutureExt;
use std::future::Future;
use std::pin::Pin;
use std::task::{Context, Poll};
use ton_config::TonConfig;
use tower::Service;

#[derive(Default, Debug, Clone)]
pub struct MakeTonlibjsonClient;

impl Service<TonConfig> for MakeTonlibjsonClient {
    type Response = TonlibjsonClient;
    type Error = anyhow::Error;
    type Future = Pin<Box<dyn Future<Output = Result<Self::Response, Self::Error>> + Send>>;

    fn poll_ready(&mut self, _: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        Poll::Ready(Ok(()))
    }

    fn call(&mut self, req: TonConfig) -> Self::Future {
        ClientBuilder::from_config(req)
            .disable_logging()
            .build()
            .boxed()
    }
}

struct ClientBuilder {
    config: TonConfig,
    logging: Option<i32>,
}

impl ClientBuilder {
    fn from_config(config: TonConfig) -> Self {
        Self {
            config,
            logging: None,
        }
    }

    fn disable_logging(mut self) -> Self {
        self.logging = Some(0);

        self
    }

    async fn build(self) -> anyhow::Result<TonlibjsonClient> {
        if let Some(level) = self.logging {
            TonlibjsonClient::set_logging(level);
        }

        TonlibjsonClient::new(self.config).context("build TonlibjsonClient")
    }
}
