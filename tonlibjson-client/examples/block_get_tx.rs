use futures::{StreamExt, TryStreamExt};
// use std::sync::{Arc, Mutex};
// use futures::{stream, StreamExt};
// use tokio::sync::RwLock;
use tokio::time::Instant;
use tonlibjson_client::ton::TonClient;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt::init();

    let mut ton = TonClient::from_env().await?;

    ton.ready().await?;

    // let block = ton.get_masterchain_info().await?.last;

    // let block = ton.get_shards_by_block_id(block.clone()).await?
    //     .first().unwrap().to_owned();

    let block = ton.look_up_block_by_seqno(0, -9223372036854775808, 34716987).await?;

    tracing::info!("run");

    let now = Instant::now();
    let txs = ton.get_block_tx_stream(&block, false).try_collect::<Vec<_>>().await;
    let elapsed = now.elapsed();
    // tracing::info!(txs = ?txs);

    tracing::info!("Elapsed: {:.2?}", elapsed);

    let now = Instant::now();
    let txs = ton.get_block_tx_stream(&block, true).try_collect::<Vec<_>>().await;
    let elapsed = now.elapsed();
    // tracing::info!(txs = ?txs);

    tracing::info!("Elapsed: {:.2?}", elapsed);

    let now = Instant::now();
    let txs = ton.get_block_tx_stream_unordered(&block).try_collect::<Vec<_>>().await;
    let elapsed = now.elapsed();
    // tracing::info!(txs = ?txs);

    tracing::info!("Elapsed: {:.2?}", elapsed);

    let now = Instant::now();
    let accounts = ton.get_accounts_in_block_stream(&block).try_collect::<Vec<_>>().await?;
    let elapsed = now.elapsed();

    tracing::info!(accounts = ?accounts, elapsed = ?elapsed);

    // let max = Arc::new(RwLock::new(0));
    //
    // tracing::info!(from = block.seqno - 100000, to = block.seqno);
    //
    // let _ = stream::iter((block.seqno - 100000 .. block.seqno).rev())
    //     .for_each_concurrent(1000, |seqno| {
    //         let max = max.clone();
    //         let ton = ton.clone();
    //
    //         async move {
    //             let Ok(block_id) = ton.look_up_block_by_seqno(block.workchain, block.shard, seqno).await else {
    //                 return;
    //             };
    //
    //             let tx_count = ton.get_tx_stream(block_id).count().await;
    //
    //             if tx_count > *(max.read().await) {
    //                 *max.write().await = tx_count;
    //                 tracing::info!(count = tx_count, seqno = seqno, "new max")
    //             }
    //         }
    //     }).await;

    Ok(())
}
