use crate::client::TonlibjsonClient;
use std::future::{Ready, ready};
use std::task::{Context, Poll};
use ton_config::TonConfig;
use tower::Service;

#[derive(Default, Debug, Clone)]
pub struct MakeTonlibjsonClient;

impl Service<TonConfig> for MakeTonlibjsonClient {
    type Response = TonlibjsonClient;
    type Error = anyhow::Error;
    type Future = Ready<Result<Self::Response, Self::Error>>;

    fn poll_ready(&mut self, _: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        Poll::Ready(Ok(()))
    }

    fn call(&mut self, req: TonConfig) -> Self::Future {
        let client = ClientBuilder::from_config(req).disable_logging().build();

        ready(client)
    }
}

struct ClientBuilder {
    config: TonConfig,
    logging: Option<i32>,
}

impl ClientBuilder {
    fn from_config(config: TonConfig) -> Self {
        Self {
            config: config.clone(),
            logging: None,
        }
    }

    fn disable_logging(mut self) -> Self {
        self.logging = Some(0);

        self
    }

    fn build(self) -> anyhow::Result<TonlibjsonClient> {
        if let Some(level) = self.logging {
            TonlibjsonClient::set_logging(level);
        }

        TonlibjsonClient::new(self.config)
    }
}
