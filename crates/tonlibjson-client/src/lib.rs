mod adapter;
mod client;
mod make;
pub mod tl;

pub use crate::adapter::{TonlibjsonAdapter, make::MakeTonlibjsonAdapter};
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
    async fn oneshot_should_complete_query() -> anyhow::Result<()> {
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
            server.liteserver_stop().await?;
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
        server.liteserver_stop().await?;

        let response = timeout(
            Duration::from_secs(30),
            (&mut adapter).oneshot(GetMasterchainInfo::default()),
        )
        .await?;
        let readiness = ServiceExt::<GetMasterchainInfo>::ready(&mut adapter).await;

        assert_eq!(
            response.unwrap_err().to_string(),
            "Ton error occurred with code 500, message LITE_SERVER_NETWORK"
        );
        assert_eq!(readiness.unwrap_err().to_string(), "connection is closed");
        Ok(())
    }

    #[tokio::test]
    #[tracing_test::traced_test]
    async fn disconnect_should_invalidate_all_clones() -> anyhow::Result<()> {
        let (server, mut adapter) = setup().await?;
        let mut clone = adapter.clone();
        <TonlibjsonAdapter as ServiceExt<GetMasterchainInfo>>::ready(&mut adapter).await?;
        <TonlibjsonAdapter as ServiceExt<GetMasterchainInfo>>::ready(&mut clone).await?;
        server.liteserver_stop().await?;

        let response = timeout(
            Duration::from_secs(30),
            (&mut adapter).oneshot(GetMasterchainInfo::default()),
        )
        .await?;
        let readiness = ServiceExt::<GetMasterchainInfo>::ready(&mut clone).await;

        assert_eq!(
            response.unwrap_err().to_string(),
            "Ton error occurred with code 500, message LITE_SERVER_NETWORK"
        );
        assert_eq!(readiness.unwrap_err().to_string(), "connection is closed");
        Ok(())
    }

    #[tokio::test]
    #[tracing_test::traced_test]
    async fn request_should_fail_with_timeout_when_server_hangs() -> anyhow::Result<()> {
        let (server, mut adapter) = setup().await?;

        server.liteserver_pause().await?;
        let request = timeout(
            Duration::from_secs(30),
            (&mut adapter).oneshot(GetMasterchainInfo::default()),
        )
        .await?;
        let readiness = ServiceExt::<GetMasterchainInfo>::ready(&mut adapter).await;

        assert_eq!(
            request.unwrap_err().to_string(),
            "Ton error occurred with code 500, message LITE_SERVER_NETWORKtimeout for adnl query query"
        );
        assert!(readiness.is_ok());

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
