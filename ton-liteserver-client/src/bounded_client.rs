use std::future::{ready, Ready};
use std::task::{Context, Poll};
use anyhow::anyhow;
use futures::future::{BoxFuture, FutureExt, TryFutureExt};
use tower::Service;
use crate::client::LiteServerClient;
use crate::request::TargetBlockId;
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

impl<R> Service<R> for BoundedClient where R: TargetBlockId {
    type Response = R::Response;
    type Error = anyhow::Error;
    type Future = BoxFuture<'static, Result<Self::Response, Self::Error>>;

    fn poll_ready(&mut self, cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        match self.upper_bound_watcher.current_upper_bound().as_ref() {
            None => {
                cx.waker().wake_by_ref();

                Poll::Pending
            },
            Some(_) => {
                <LiteServerClient as Service<R>>::poll_ready(&mut self.inner, cx)
                    .map_err(anyhow::Error::from)
            }
        }
    }

    fn call(&mut self, req: R) -> Self::Future {
        let upper_bound_ref = self.upper_bound_watcher
            .current_upper_bound();

        let upper_bound = upper_bound_ref
            .as_ref()
            .expect("call before ready");

        if req.target_block_id() > &upper_bound.last {
            return ready(Err(anyhow!("seqno not available"))).boxed();
        }

        <LiteServerClient as Service<R>>::call(&mut self.inner, req)
            .map_err(anyhow::Error::from)
            .boxed()
    }
}


#[cfg(test)]
mod tests {
    use tracing_test::traced_test;
    use tower::ServiceExt;
    use crate::tl::{LiteServerGetBlockHeader};
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

    #[tokio::test]
    #[traced_test]
    #[ignore]
    async fn bounded_client_get_block_header_test() -> anyhow::Result<()> {
        let client = crate::client::tests::provided_client().await?;
        let mut client = BoundedClient::new(client);
        let result = (&mut client).oneshot(LiteServerGetMasterchainInfo::default()).await.unwrap();

        let result = (&mut client).oneshot(LiteServerGetBlockHeader {
            id: result.last.into(),
            mode: 0,
        }).await;


        assert!(result.is_ok());
        tracing::info!(result =? result.unwrap());
        Ok(())
    }

    #[tokio::test]
    #[traced_test]
    #[ignore]
    async fn bounded_client_get_block_header_test() -> anyhow::Result<()> {
        let client = crate::client::tests::provided_client().await?;
        let mut client = BoundedClient::new(client);
        let result = (&mut client).oneshot(LiteServerGetMasterchainInfo::default()).await.unwrap();

        let result = (&mut client).oneshot(LiteServerGetBlockHeader {
            id: result.last.into(),
            mode: 0,
        }).await;


        assert!(result.is_ok());
        tracing::info!(result =? result.unwrap());
        Ok(())
    }
}
