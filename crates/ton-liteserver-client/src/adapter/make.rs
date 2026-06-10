use crate::adapter::LiteServerAdapter;
use crate::client::LiteServerClient;
use adnl_tcp::client::ServerKey;
use anyhow::anyhow;
use base64::Engine;
use futures::FutureExt;
use futures::future::BoxFuture;
use std::task::{Context, Poll};
use ton_config::TonConfig;
use tower::Service;

#[derive(Default, Debug, Clone)]
pub struct MakeLiteServerAdapter;

impl Service<TonConfig> for MakeLiteServerAdapter {
    type Response = LiteServerAdapter;
    type Error = anyhow::Error;
    type Future = BoxFuture<'static, Result<Self::Response, Self::Error>>;

    fn poll_ready(&mut self, _: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        Poll::Ready(Ok(()))
    }

    fn call(&mut self, config: TonConfig) -> Self::Future {
        async move {
            let liteserver = config
                .liteservers
                .first()
                .ok_or_else(|| anyhow!("ton config does not contain any liteservers"))?;

            let addr = liteserver.addr;
            let key: ServerKey = base64::engine::general_purpose::STANDARD
                .decode(&liteserver.id.key)?
                .as_slice()
                .try_into()
                .map_err(|_| anyhow!("invalid liteserver key length"))?;

            let inner = LiteServerClient::connect(addr, key).await?;
            tracing::info!("connected to liteserver at {}", addr);

            Ok(LiteServerAdapter::new(inner))
        }
        .boxed()
    }
}
