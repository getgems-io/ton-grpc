use testcontainers_ton::LocalLiteServer;
use ton_liteserver_client::client::LiteServerClient;
use ton_liteserver_client::tl::{
    LiteServerGetMasterchainInfo, LiteServerListBlockTransactions, LiteServerLookupBlock,
    TonNodeBlockId, True,
};
use tower::ServiceExt;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt::init();

    let lite_server = LocalLiteServer::new().await?;
    let mut client =
        LiteServerClient::connect(lite_server.addr(), lite_server.server_key()).await?;

    let last = (&mut client)
        .oneshot(LiteServerGetMasterchainInfo::default())
        .await?
        .last;
    tracing::info!(?last);

    let partial_block_id = TonNodeBlockId {
        workchain: last.workchain,
        shard: last.shard,
        seqno: last.seqno - 200000,
    };
    let block_id = (&mut client)
        .oneshot(LiteServerLookupBlock {
            mode: 1,
            id: partial_block_id,
            lt: None,
            utime: None,
        })
        .await?;
    tracing::info!(?block_id);

    let txs = (&mut client)
        .oneshot(LiteServerListBlockTransactions {
            id: last,
            mode: 15,
            count: 4,
            after: None,
            reverse_order: None,
            want_proof: Some(True {}),
        })
        .await?;
    for tx in txs.ids {
        tracing::info!(?tx)
    }

    let txs = (&mut client)
        .oneshot(LiteServerListBlockTransactions {
            id: block_id.id.clone(),
            mode: 1,
            count: 4,
            after: None,
            reverse_order: None,
            want_proof: None,
        })
        .await?;
    for tx in txs.ids {
        tracing::info!(?tx)
    }

    let txs = (&mut client)
        .oneshot(LiteServerListBlockTransactions {
            id: block_id.id,
            mode: 1,
            count: 4,
            after: None,
            reverse_order: Some(True {}),
            want_proof: None,
        })
        .await?;
    for tx in txs.ids {
        tracing::info!(?tx)
    }

    Ok(())
}
