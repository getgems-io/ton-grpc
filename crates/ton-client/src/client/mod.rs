use crate::RequestHandler;
use std::task::{Context, Poll};
use ton_tower::request::GetMasterchainInfo;
use tower::Service;
use tower::load::Load;

pub mod account_client;
pub mod block_client;
pub mod client_ext;
pub mod message_client;
pub mod smc_client;

#[derive(Debug, Clone)]
pub struct Client<S> {
    inner: S,
}

impl<S> Client<S> {
    pub fn new(inner: S) -> Self {
        Self { inner }
    }

    pub fn get_ref(&self) -> &S {
        &self.inner
    }

    pub fn get_mut(&mut self) -> &mut S {
        &mut self.inner
    }

    pub fn into_inner(self) -> S {
        self.inner
    }

    pub async fn wait_ready(&mut self) -> anyhow::Result<()>
    where
        S: RequestHandler<GetMasterchainInfo>,
    {
        self.get_masterchain_info().await?;
        Ok(())
    }
}

impl<R, S> Service<R> for Client<S>
where
    S: Service<R>,
{
    type Response = S::Response;
    type Error = S::Error;
    type Future = S::Future;

    fn poll_ready(&mut self, cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        self.inner.poll_ready(cx)
    }

    fn call(&mut self, req: R) -> Self::Future {
        self.inner.call(req)
    }
}

impl<S> Load for Client<S>
where
    S: Load,
{
    type Metric = S::Metric;

    fn load(&self) -> Self::Metric {
        self.inner.load()
    }
}
