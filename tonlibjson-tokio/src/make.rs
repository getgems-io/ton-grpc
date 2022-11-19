use std::future::Future;
use std::pin::Pin;
use std::task::{Context, Poll};
use tower::limit::{ConcurrencyLimit, ConcurrencyLimitLayer};
use tower::{Layer, Service};
use tracing::{debug, warn};
use crate::client::Client;
use crate::request::Request;
use crate::{ClientBuilder, GetMasterchainInfo};
use crate::ton_config::TonConfig;


#[derive(Default, Debug)]
pub struct  ClientFactory;

impl Service<TonConfig> for ClientFactory {
    type Response = ConcurrencyLimit<Client>;
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

            let _ = client.call(Request::new(serde_json::to_value(GetMasterchainInfo {})?)).await?;

            let client = ConcurrencyLimitLayer::new(100)
                .layer(client);

            debug!("successfully made new client");

            anyhow::Ok(client)
        })
    }
}
