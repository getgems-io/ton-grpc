use futures::TryStreamExt;
use testcontainers_ton::LocalLiteServer;
use ton_client::Client;
use ton_liteserver_client::LiteServerAdapter;
use ton_liteserver_client::client::LiteServerClient;
use ton_tower::request::GetTransactions;
use ton_tower::response::{BlockIdExt, BlockTransactionsExt, Transaction};
use tonlibjson_client::MakeTonlibjsonAdapter;
use tower::{Service, ServiceExt};

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt::init();

    let server = LocalLiteServer::new().await?;
    let adapter = MakeTonlibjsonAdapter
        .oneshot(server.config().clone())
        .await?;
    let mut tonlib = Client::new(adapter);
    tonlib.wait_ready().await?;
    tracing::info!("tonlib client ready");

    let liteserver_inner = LiteServerClient::connect(server.addr(), server.server_key()).await?;
    let mut liteserver = Client::new(LiteServerAdapter::new(liteserver_inner));
    tracing::info!("liteserver client ready");

    let master2 = liteserver.get_masterchain_info().await?;
    let mut master;
    loop {
        master = tonlib.get_masterchain_info().await?;
        tracing::info!(seqno = master.last.seqno, "masterchain info");
        if master.last.seqno == master2.last.seqno {
            break;
        } else if master2.last.seqno < master.last.seqno {
            return Err(anyhow::anyhow!("masterchain info is behind"));
        }

        tokio::time::sleep(std::time::Duration::from_secs(1)).await;
    }

    assert_eq!(master, master2, "masterchain info mismatch");

    let block = master.last;
    compare_block_transactions(&tonlib, &liteserver, &block).await?;

    let shards = tonlib.get_shards_by_block_id(block).await?;
    for shard in &shards {
        compare_block_transactions(&tonlib, &liteserver, shard).await?;
    }

    tracing::info!("all transactions match");

    Ok(())
}

async fn compare_block_transactions<A, B>(
    tonlib: &Client<A>,
    liteserver: &Client<B>,
    block: &BlockIdExt,
) -> anyhow::Result<()>
where
    A: Service<GetTransactions, Response = BlockTransactionsExt, Error = anyhow::Error>
        + Clone
        + Send
        + Sync
        + 'static,
    A::Future: Send,
    B: Service<GetTransactions, Response = BlockTransactionsExt, Error = anyhow::Error>
        + Clone
        + Send
        + Sync
        + 'static,
    B::Future: Send,
{
    let tonlib_txs: Vec<Transaction> = tonlib
        .get_block_tx_stream(block, false)
        .try_collect()
        .await?;

    let liteserver_txs: Vec<Transaction> = liteserver
        .get_block_tx_stream(block, false)
        .try_collect()
        .await?;

    tracing::info!(
        workchain = block.workchain,
        shard = block.shard,
        seqno = block.seqno,
        tonlib = tonlib_txs.len(),
        liteserver = liteserver_txs.len(),
        "comparing block transactions"
    );

    assert_eq!(
        tonlib_txs.len(),
        liteserver_txs.len(),
        "transaction count mismatch for block {}:{}:{}",
        block.workchain,
        block.shard,
        block.seqno
    );

    for (t, l) in tonlib_txs.iter().zip(liteserver_txs.iter()) {
        assert_eq!(t.transaction_id, l.transaction_id);
        assert_eq!(t.address, l.address);
        assert_eq!(t.utime, l.utime);
        assert_eq!(t.fee, l.fee);
        assert_eq!(t.data, l.data);
        assert_eq!(t.in_msg.is_some(), l.in_msg.is_some());
        if let (Some(tm), Some(lm)) = (&t.in_msg, &l.in_msg) {
            assert_eq!(tm.source, lm.source);
            assert_eq!(tm.destination, lm.destination);
            assert_eq!(tm.value, lm.value);
        }
        assert_eq!(t.out_msgs.len(), l.out_msgs.len());
        for (tm, lm) in t.out_msgs.iter().zip(l.out_msgs.iter()) {
            assert_eq!(tm.source, lm.source);
            assert_eq!(tm.destination, lm.destination);
            assert_eq!(tm.value, lm.value);
        }
    }

    Ok(())
}
