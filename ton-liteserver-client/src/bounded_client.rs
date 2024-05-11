use std::future::{ready, Ready};
use std::task::{Context, Poll};
use tower::Service;
use crate::client::LiteServerClient;
use crate::tl::{LiteServerBoxedMasterchainInfo, LiteServerGetMasterchainInfo};
use crate::upper_bound_watcher::UpperBoundWatcher;

pub struct BoundedClient {
    inner: LiteServerClient,
    upper_bound_watcher: UpperBoundWatcher,
}

impl BoundedClient {
    pub fn new(inner: LiteServerClient) -> Self {
        let upper_bound_watcher = UpperBoundWatcher::new(inner.clone());

        Self { inner, upper_bound_watcher }
    }
}

impl Service<LiteServerGetMasterchainInfo> for BoundedClient {
    type Response = LiteServerBoxedMasterchainInfo;
    type Error = crate::client::Error;
    type Future = Ready<Result<Self::Response, Self::Error>>;

    fn poll_ready(&mut self, cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        match self.upper_bound_watcher.current_upper_bound().as_ref() {
            None => {
                cx.waker().wake_by_ref();

                Poll::Pending
            },
            Some(_) => {
                <LiteServerClient as Service<LiteServerGetMasterchainInfo>>::poll_ready(&mut self.inner, cx)
            }
        }
    }

    fn call(&mut self, _: LiteServerGetMasterchainInfo) -> Self::Future {
        match self.upper_bound_watcher.current_upper_bound().as_ref() {
            None => { unreachable!() }
            Some(info) => {
                ready(Ok(info.clone()))
            }
        }
    }
}


#[cfg(test)]
mod tests {
    use tracing_test::traced_test;
    use tower::ServiceExt;
    use super::*;

    #[tokio::test]
    #[traced_test]
    #[ignore]
    async fn bounded_client_masterchain_info_test() -> anyhow::Result<()> {
        let client = crate::client::tests::provided_client().await?;
        let mut client = BoundedClient::new(client);

        let result = (&mut client).oneshot(LiteServerGetMasterchainInfo::default()).await;

        assert!(result.is_ok());
        Ok(())
    }
}
