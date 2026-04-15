use futures::TryStreamExt;
use testcontainers_ton::LocalLiteServer;
use ton_client::{BlockClient, BlockClientExt as _, BlockIdExt, Transaction};
use ton_liteserver_client::client::LiteServerClient;
use tonlibjson_client::ton::TonClientBuilder;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt::init();

    let server = LocalLiteServer::new().await?;

    let mut tonlib = TonClientBuilder::from_config(server.config()).build()?;
    tonlib.ready().await?;
    tracing::info!("tonlib client ready");

    let liteserver = LiteServerClient::connect(server.addr(), server.server_key()).await?;
    tracing::info!("liteserver client ready");

    let master2 = liteserver.get_masterchain_info().await?;
    tokio::time::sleep(std::time::Duration::from_secs(2)).await;
    let master = tonlib.get_masterchain_info().await?;
    tracing::info!(seqno = master.last.seqno, "masterchain info");

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

async fn compare_block_transactions(
    tonlib: &impl BlockClient,
    liteserver: &impl BlockClient,
    block: &BlockIdExt,
) -> anyhow::Result<()> {
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
        assert_eq!(
            t.transaction_id, l.transaction_id,
            "transaction_id mismatch: tonlib={:?} liteserver={:?}",
            t.transaction_id, l.transaction_id
        );

        assert_eq!(
            t.address, l.address,
            "address mismatch for tx lt={}",
            t.transaction_id.lt
        );

        assert_eq!(
            t.utime, l.utime,
            "utime mismatch for tx lt={}",
            t.transaction_id.lt
        );

        assert_eq!(
            t.fee, l.fee,
            "fee mismatch for tx lt={}",
            t.transaction_id.lt
        );

        assert_eq!(
            t.data, l.data,
            "data mismatch for tx lt={}",
            t.transaction_id.lt
        );

        assert_eq!(
            t.in_msg.is_some(),
            l.in_msg.is_some(),
            "in_msg presence mismatch for tx lt={}",
            t.transaction_id.lt
        );

        if let (Some(tm), Some(lm)) = (&t.in_msg, &l.in_msg) {
            assert_eq!(
                tm.source, lm.source,
                "in_msg source mismatch for tx lt={}",
                t.transaction_id.lt
            );
            assert_eq!(
                tm.destination, lm.destination,
                "in_msg destination mismatch for tx lt={}",
                t.transaction_id.lt
            );
            assert_eq!(
                tm.value, lm.value,
                "in_msg value mismatch for tx lt={}",
                t.transaction_id.lt
            );
        }

        assert_eq!(
            t.out_msgs.len(),
            l.out_msgs.len(),
            "out_msgs count mismatch for tx lt={}",
            t.transaction_id.lt
        );

        for (i, (tm, lm)) in t.out_msgs.iter().zip(l.out_msgs.iter()).enumerate() {
            assert_eq!(
                tm.source, lm.source,
                "out_msg[{i}] source mismatch for tx lt={}",
                t.transaction_id.lt
            );
            assert_eq!(
                tm.destination, lm.destination,
                "out_msg[{i}] destination mismatch for tx lt={}",
                t.transaction_id.lt
            );
            assert_eq!(
                tm.value, lm.value,
                "out_msg[{i}] value mismatch for tx lt={}",
                t.transaction_id.lt
            );
        }

        tracing::info!(
            lt = t.transaction_id.lt,
            address = %t.address,
            "tx matched"
        );
    }

    Ok(())
}
