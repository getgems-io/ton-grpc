use tokio::time::Instant;
use tonlibjson_client::ton::TonClient;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt::init();

    let mut ton = TonClient::from_env().await?;

    ton.ready().await?;

    let block = ton.get_masterchain_info().await?.last;

    let block = ton.get_shards_by_block_id(block.clone()).await?
        .first().unwrap().to_owned();

    tracing::info!("run");

    let now = Instant::now();
    let txs = ton.blocks_get_transactions_verified(&block, None).await?;
    tracing::info!("{:?}", txs);
    let elapsed = now.elapsed();

    tracing::info!("Elapsed: {:.2?}", elapsed);

    let now = Instant::now();
    let txs = ton.blocks_get_transactions(&block, None).await?;
    tracing::info!("{:?}", txs);
    let elapsed = now.elapsed();

    tracing::info!("Elapsed: {:.2?}", elapsed);

    Ok(())
}
