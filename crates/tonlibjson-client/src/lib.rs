pub mod adapter;
mod client;
mod cursor;
mod deserialize;
mod error;
mod make;
mod metric;
mod request;
mod retry;
mod session;
pub mod tl;
pub mod ton;

pub use crate::adapter::{TonlibjsonAdapter, make::MakeTonlibjsonAdapter};
pub use crate::{client::TonlibjsonClient, make::MakeTonlibjsonClient};

#[cfg(test)]
mod integration {
    use crate::{MakeTonlibjsonAdapter, TonlibjsonAdapter};
    use testcontainers_ton::LocalLiteServer;
    use ton_tower::request::{GetBlockHeader, GetMasterchainInfo};
    use tower::ServiceExt;

    #[tokio::test]
    async fn should_get_masterchain_info() -> anyhow::Result<()> {
        let (_server, adapter) = setup().await?;

        let masterchain_info = adapter.oneshot(GetMasterchainInfo::default()).await?;

        assert!(masterchain_info.last.seqno > 0);

        Ok(())
    }

    #[tokio::test]
    async fn should_get_block_header() -> anyhow::Result<()> {
        let (_server, adapter) = setup().await?;

        let info = adapter
            .clone()
            .oneshot(GetMasterchainInfo::default())
            .await?;
        let header = adapter.oneshot(GetBlockHeader { id: info.last }).await?;

        assert_eq!(header.id.workchain, -1);

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
