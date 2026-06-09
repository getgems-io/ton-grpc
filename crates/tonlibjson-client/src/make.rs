use crate::client::TonlibjsonClient;
use crate::tl::BlocksGetMasterchainInfo;
use serde_json::{Value, json};
use std::future::Future;
use std::pin::Pin;
use std::task::{Context, Poll};
use ton_config::TonConfig;
use tower::{Service, ServiceExt};

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
        Box::pin(async move {
            let mut client = ClientBuilder::from_config(&req)
                .disable_logging()
                .build()
                .await?;

            let _ = (&mut client)
                .oneshot(BlocksGetMasterchainInfo::default())
                .await?;

            Ok(client)
        })
    }
}

struct ClientBuilder {
    config: Value,
    logging: Option<i32>,
}

impl ClientBuilder {
    fn from_config(config: &TonConfig) -> Self {
        let full_config = json!({
            "@type": "init",
            "options": {
                "@type": "options",
                "config": {
                    "@type": "config",
                    "config": config.to_string(),
                    "use_callbacks_for_network": false,
                    "blockchain_name": "",
                    "ignore_cache": true
                },
                "keystore_type": {
                    "@type": "keyStoreTypeInMemory"
                }
            }
        });

        Self {
            config: full_config,
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

        let mut client = TonlibjsonClient::new();
        let _ = (&mut client).oneshot(self.config).await?;

        Ok(client)
    }
}
