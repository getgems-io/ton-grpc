use futures::stream::StreamExt;
use ton_client::Client;
use ton_config::load_ton_config;
use ton_tower::response::Transaction;
use tonlibjson_client::{MakeTonlibjsonAdapter, TonlibjsonAdapter};
use tower::ServiceExt;
use url::Url;

async fn client() -> Client<TonlibjsonAdapter> {
    let config = load_ton_config(
        Url::parse("https://github.com/ton-blockchain/ton-blockchain.github.io/raw/7f6526fb2635eb514065beb04ee902ded5dd8a7b/global.config.json").unwrap()
    )
    .await
    .unwrap();
    let adapter = MakeTonlibjsonAdapter.oneshot(config).await.unwrap();
    let mut client = Client::new(adapter);

    client.wait_ready().await.unwrap();

    client
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt::init();

    let mut client = client().await;

    let block = client
        .look_up_block_by_seqno(0, 576460752303423488, 40486536)
        .await?;

    tracing::info!(block = ?block);

    let txs = client
        .get_block_tx_stream(&block, false)
        .collect::<Vec<anyhow::Result<Transaction>>>()
        .await;

    tracing::info!( txs_count = ?txs.len());

    Ok(())
}
