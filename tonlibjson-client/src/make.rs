use std::future::Future;
use std::pin::Pin;
use std::task::{Context, Poll};
use serde_json::{json, Value};
use tower::limit::{ConcurrencyLimit, ConcurrencyLimitLayer};
use tower::{Layer, Service, ServiceExt};
use tracing::{debug, warn};
use crate::block::GetMasterchainInfo;
use crate::client::Client;
use crate::request::Request;
use crate::session::{SessionClient, SessionRequest};
use crate::ton_config::TonConfig;


#[derive(Default, Debug)]
pub struct  ClientFactory;

impl Service<TonConfig> for ClientFactory {
    type Response = ConcurrencyLimit<SessionClient>;
    type Error = anyhow::Error;
    type Future = Pin<Box<dyn Future<Output = Result<Self::Response, Self::Error>> + Send>>;

    fn poll_ready(&mut self, _: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        Poll::Ready(Ok(()))
    }

    fn call(&mut self, req: TonConfig) -> Self::Future {
        Box::pin(async move {
            warn!("make new liteserver");

            let mut client = ClientBuilder::from_config(&req.to_string())
                .disable_logging()
                .build()
                .await?;

            let _ = client.call(Request::new(GetMasterchainInfo {})?).await?;

            client.setup_first_available_block().await?;

            let client = SessionClient::new(client);

            let mut client = ConcurrencyLimitLayer::new(100)
                .layer(client);

            debug!("successfully made new client");

            anyhow::Ok(client)
        })
    }
}


struct ClientBuilder {
    config: Value,
    disable_logging: Option<Value>,
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
            disable_logging: None,
        }
    }

    fn disable_logging(&mut self) -> &mut Self {
        self.disable_logging = Some(json!({
            "@type": "setLogVerbosityLevel",
            "new_verbosity_level": 0
        }));

        self
    }

    async fn build(&self) -> anyhow::Result<Client> {
        let mut client = Client::new();
        if let Some(ref disable_logging) = self.disable_logging {
            client.call(Request::new(disable_logging.clone())?).await?;
        }

        client.call(Request::new(self.config.clone())?).await?;

        Ok(client)
    }
}
