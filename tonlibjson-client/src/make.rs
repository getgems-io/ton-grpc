use crate::block::BlocksGetMasterchainInfo;
use crate::client::Client;
use crate::cursor::client::CursorClient;
use crate::error::ErrorLayer;
use serde_json::{json, Value};
use std::future::Future;
use std::pin::Pin;
use std::task::{Context, Poll};
use std::time::Duration;
use ton_client_util::discover::config::{LiteServerId, TonConfig};
use ton_client_util::service::shared::SharedLayer;
use ton_client_util::service::timeout::TimeoutLayer;
use tower::limit::ConcurrencyLimitLayer;
use tower::load::PeakEwma;
use tower::{Service, ServiceBuilder, ServiceExt};

#[derive(Default, Debug)]
pub(crate) struct ClientFactory;

impl Service<TonConfig> for ClientFactory {
    type Response = Client;
    type Error = anyhow::Error;
    type Future = Pin<Box<dyn Future<Output = Result<Self::Response, Self::Error>> + Send>>;

    fn poll_ready(&mut self, _: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        Poll::Ready(Ok(()))
    }

    fn call(&mut self, req: TonConfig) -> Self::Future {
        Box::pin(async move {
            let mut client = ClientBuilder::from_config(&req.to_string())
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

#[derive(Default, Debug, Copy, Clone)]
pub(crate) struct CursorClientFactory;

impl CursorClientFactory {
    pub(crate) fn create(id: LiteServerId, client: PeakEwma<Client>) -> CursorClient {
        ServiceBuilder::new()
            .layer_fn(|s| CursorClient::new(id.to_string(), s))
            .layer(ConcurrencyLimitLayer::new(256))
            .layer(SharedLayer)
            .layer(ErrorLayer)
            .layer(TimeoutLayer::new(Duration::from_secs(5)))
            .service(client)
    }
}

struct ClientBuilder {
    config: Value,
    logging: Option<i32>,
}

impl ClientBuilder {
    fn from_config(config: &str) -> Self {
        let full_config = json!({
            "@type": "init",
            "options": {
                "@type": "options",
                "config": {
                    "@type": "config",
                    "config": config,
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

    async fn build(self) -> anyhow::Result<Client> {
        if let Some(level) = self.logging {
            Client::set_logging(level);
        }

        let mut client = Client::new();
        let _ = (&mut client).oneshot(self.config).await?;

        Ok(client)
    }
}
