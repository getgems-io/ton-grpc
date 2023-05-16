use std::future::{Future, ready, Ready};
use std::pin::Pin;
use std::task::{Context, Poll};
use std::time::Duration;
use serde_json::{json, Value};
use tower::limit::{ConcurrencyLimitLayer};
use tower::{Layer, Service, ServiceExt};
use tower::load::{PeakEwma};
use tracing::{debug};
use crate::block::GetMasterchainInfo;
use crate::client::Client;
use crate::cursor_client::CursorClient;
use crate::session::SessionClient;
use crate::ton_config::TonConfig;
use crate::request::{Callable, Request, RequestBody};

#[derive(Default, Debug)]
pub struct ClientFactory;

impl Service<TonConfig> for ClientFactory {
    type Response = Client;
    type Error = anyhow::Error;
    type Future = Pin<Box<dyn Future<Output = Result<Self::Response, Self::Error>> + Send + Sync>>;

    fn poll_ready(&mut self, _: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        Poll::Ready(Ok(()))
    }

    fn call(&mut self, req: TonConfig) -> Self::Future {
        Box::pin(async move {
            debug!("make new client");

            let mut client = ClientBuilder::from_config(&req.to_string())
                .disable_logging()
                .build()
                .await?;

            // TODO[akostylev0]
            let _ = client.ready()
                .await?
                .call(Request::new(RequestBody::GetMasterchainInfo(GetMasterchainInfo::default()), Duration::from_secs(3)))
                .await?;

            debug!("successfully made new client");

            Ok(client)
        })
    }
}

#[derive(Default, Debug, Copy, Clone)]
pub struct CursorClientFactory;

impl CursorClientFactory {
    pub fn create(client: PeakEwma<Client>) -> CursorClient {
        debug!("make new cursor client");
        let client = SessionClient::new(client);

        let client = ConcurrencyLimitLayer::new(100)
            .layer(client);

        let client = CursorClient::new(client);

        debug!("successfully made new cursor client");

        client
    }
}

impl Service<PeakEwma<Client>> for CursorClientFactory {
    type Response = CursorClient;
    type Error = anyhow::Error;
    type Future = Ready<Result<Self::Response, Self::Error>>;

    fn poll_ready(&mut self, _: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        Poll::Ready(Ok(()))
    }

    fn call(&mut self, client: PeakEwma<Client>) -> Self::Future {
        ready(Ok(Self::create(client)))
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

    fn disable_logging(&mut self) -> &mut Self {
        self.logging = Some(0);

        self
    }

    async fn build(&self) -> anyhow::Result<Client> {
        if let Some(level) = self.logging {
            Client::set_logging(level);
        }

        let mut client = Client::new();

        self.config.clone().call(&mut client).await?;

        Ok(client)
    }
}
