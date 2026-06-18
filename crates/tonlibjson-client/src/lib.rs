mod adapter;
mod client;
mod make;
pub mod tl;

pub use crate::adapter::{make::MakeTonlibjsonAdapter, TonlibjsonAdapter};
pub use crate::{client::TonlibjsonClient, make::MakeTonlibjsonClient};

#[cfg(test)]
mod integration {
    use crate::{MakeTonlibjsonAdapter, TonlibjsonAdapter};
    use std::time::Duration;
    use testcontainers_ton::LocalLiteServer;
    use tokio::time::timeout;
    use ton_tower::request::GetMasterchainInfo;
    use tower::ServiceExt;

    #[tokio::test]
    #[tracing_test::traced_test]
    async fn should_complete_query() -> anyhow::Result<()> {
        let (_server, adapter) = setup().await?;

        let masterchain_info = adapter.oneshot(GetMasterchainInfo::default()).await?;

        assert!(masterchain_info.last.seqno > 0);
        Ok(())
    }

    #[tokio::test]
    #[tracing_test::traced_test]
    async fn ready_should_fail_when_server_is_down_before_connection() -> anyhow::Result<()> {
        let config = {
            let server = LocalLiteServer::new().await?;
            server.stop().await?;
            server.config().clone()
        };
        let mut adapter = MakeTonlibjsonAdapter.oneshot(config).await?;

        let readiness = ServiceExt::<GetMasterchainInfo>::ready(&mut adapter).await;

        assert!(readiness.is_err());
        Ok(())
    }

    #[tokio::test]
    #[tracing_test::traced_test]
    async fn call_should_fail_when_server_is_down_after_connection() -> anyhow::Result<()> {
        let (server, mut adapter) = setup().await?;

        server.stop().await?;
        let request = timeout(
            Duration::from_secs(30),
            adapter.clone().oneshot(GetMasterchainInfo::default()),
        )
        .await?;
        let readiness = ServiceExt::<GetMasterchainInfo>::ready(&mut adapter).await;

        assert!(
            request.is_err(),
            "request must fail once the server is down, got: {request:?}"
        );
        assert!(
            readiness.is_err(),
            "poll_ready must report broken state after a lite server network failure"
        );
        Ok(())
    }

    #[tokio::test]
    #[tracing_test::traced_test]
    async fn should_fail_with_timeout_when_server_hangs() -> anyhow::Result<()> {
        let (server, mut adapter) = setup().await?;

        server.pause().await?;
        let request = timeout(
            Duration::from_secs(30),
            adapter.clone().oneshot(GetMasterchainInfo::default()),
        )
        .await?;
        let readiness = ServiceExt::<GetMasterchainInfo>::ready(&mut adapter).await;

        let error = request.expect_err("request must fail once the server hangs");
        assert!(
            error.to_string().contains("timeout"),
            "a hung query must surface a timeout error, got: {error}"
        );
        assert!(
            readiness.is_ok(),
            "a transient query timeout must not mark the client as broken"
        );

        Ok(())
    }

    async fn setup() -> anyhow::Result<(LocalLiteServer, TonlibjsonAdapter)> {
        let server = LocalLiteServer::new().await?;
        let adapter = MakeTonlibjsonAdapter
            .oneshot(server.config().clone())
            .await?;
        Ok((server, adapter))
    }
}
