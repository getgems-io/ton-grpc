use std::future::Future;
use std::pin::Pin;
use std::task::{Context, Poll};
use tower::Service;
use tracing::{debug, error};
use crate::client::AsyncClient;
use crate::ClientBuilder;
use crate::liteserver::LiteserverConfig;

pub struct ClientFactory;

impl Default for ClientFactory {
    fn default() -> Self {
        ClientFactory { }
    }
}

impl Service<LiteserverConfig> for ClientFactory {
    type Response = AsyncClient;
    type Error = anyhow::Error;
    type Future = Pin<Box<dyn Future<Output = Result<Self::Response, Self::Error>> + Send>>;

    fn poll_ready(&mut self, _: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        Poll::Ready(Ok(()))
    }

    fn call(&mut self, req: LiteserverConfig) -> Self::Future {
        debug!("make new liteserver {:?}", req.liteserver.identifier());

        Box::pin(async move {
            let client = ClientBuilder::from_json_config(&req.to_config()?)
                .disable_logging()
                .build()
                .await?;

            client.synchronize().await.map_err(|e| {
                error!("cannot synchronize client {:?}", req.liteserver.identifier());
                e
            })?;

            debug!("successfully made new client {:?}", req.liteserver.identifier());

            anyhow::Ok(client)
        })
    }
}
