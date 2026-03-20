use futures::stream::StreamExt;
use std::time::Duration;
use tonlibjson_client::block::RawTransaction;
use tonlibjson_client::ton::{TonClient, TonClientBuilder};
use url::Url;

async fn client() -> TonClient {
    let mut client = TonClientBuilder::from_config_url(
        Url::parse("https://github.com/ton-blockchain/ton-blockchain.github.io/raw/7f6526fb2635eb514065beb04ee902ded5dd8a7b/global.config.json").unwrap(), Duration::from_secs(60)
    )
        .disable_retry()
        .build()
        .unwrap();

    client.ready().await.unwrap();

    client
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt::init();

    let client = client().await;

    let block = client
        .look_up_block_by_seqno(0, 576460752303423488, 40486536)
        .await?;

    tracing::info!(block = ?block);

    let txs = client
        .get_block_tx_stream(&block, false)
        .collect::<Vec<anyhow::Result<RawTransaction>>>()
        .await;

    tracing::info!( txs_count = ?txs.len());

    Ok(())
}
