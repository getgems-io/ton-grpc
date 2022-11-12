use std::future::Future;
use std::pin::Pin;
use std::task::{Context, Poll};
use serde_json::json;
use tower::Service;
use tracing::{debug, error, warn};
use crate::client::AsyncClient;
use crate::{ClientBuilder, GetMasterchainInfo};
use crate::ton_config::TonConfig;


#[derive(Default, Debug)]
pub struct ClientFactory;

impl Service<TonConfig> for ClientFactory {
    type Response = AsyncClient;
    type Error = anyhow::Error;
    type Future = Pin<Box<dyn Future<Output = Result<Self::Response, Self::Error>> + Send>>;

    fn poll_ready(&mut self, _: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        Poll::Ready(Ok(()))
    }

    fn call(&mut self, req: TonConfig) -> Self::Future {
        Box::pin(async move {
            warn!("make new liteserver");

            let client = ClientBuilder::from_config(&req)
                .disable_logging()
                .build()
                .await?;

            // Ping
            let pong = client.execute(json!(GetMasterchainInfo {})).await?;
            warn!("Pong: {}", pong);

            client.synchronize().await.map_err(|e| {
                error!("cannot synchronize client, error is {:?}", e);
                e
            })?;

            debug!("successfully made new client");

            anyhow::Ok(client)
        })
    }
}
