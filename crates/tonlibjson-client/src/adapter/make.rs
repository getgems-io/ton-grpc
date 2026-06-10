use crate::TonlibjsonAdapter;
use crate::make::MakeTonlibjsonClient;
use futures::TryFutureExt;
use std::future::Future;
use std::pin::Pin;
use std::task::{Context, Poll};
use ton_config::TonConfig;
use tower::Service;

#[derive(Default, Debug, Clone)]
pub struct MakeTonlibjsonAdapter;

impl Service<TonConfig> for MakeTonlibjsonAdapter {
    type Response = TonlibjsonAdapter;
    type Error = anyhow::Error;
    type Future = Pin<Box<dyn Future<Output = Result<Self::Response, Self::Error>> + Send>>;

    fn poll_ready(&mut self, cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        MakeTonlibjsonClient.poll_ready(cx)
    }

    fn call(&mut self, req: TonConfig) -> Self::Future {
        Box::pin(
            MakeTonlibjsonClient
                .call(req)
                .map_ok(TonlibjsonAdapter::new),
        )
    }
}
