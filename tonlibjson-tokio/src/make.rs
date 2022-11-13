use std::future::Future;
use std::pin::Pin;
use std::task::{Context, Poll};
use std::time::Duration;
use serde_json::json;
use tower::limit::{ConcurrencyLimit, ConcurrencyLimitLayer};
use tower::{Layer, Service};
use tracing::{debug, warn};
use crate::client::AsyncClient;
use crate::{ClientBuilder, GetMasterchainInfo, Request};
use crate::ton_config::TonConfig;


#[derive(Default, Debug)]
pub struct ClientFactory;

impl Service<TonConfig> for ClientFactory {
    type Response = ConcurrencyLimit<AsyncClient>;
    type Error = anyhow::Error;
    type Future = Pin<Box<dyn Future<Output = Result<Self::Response, Self::Error>> + Send>>;

    fn poll_ready(&mut self, _: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        Poll::Ready(Ok(()))
    }

    fn call(&mut self, req: TonConfig) -> Self::Future {
        Box::pin(async move {
            warn!("make new liteserver");

            let mut client = ClientBuilder::from_config(&req)
                .disable_logging()
                .build()
                .await?;

            // Ping
            let pong = client.call(
                Request::with_timeout(
                    json!(GetMasterchainInfo {}),
                    Duration::from_secs(1)
                )).await?;
            warn!("Pong: {}", pong);

            let sync = Request::with_timeout(json!({
                "@type": "sync"
            }), Duration::from_secs(60 * 5));

            client.call(sync).await?;

            debug!("successfully made new client");

            let client = ConcurrencyLimitLayer::new(100).layer(client);

            anyhow::Ok(client)
        })
    }
}
