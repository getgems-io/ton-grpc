use tonlibjson_client::ton::{TonClient, TonClientBuilder};

async fn client() -> TonClient {
    let mut client = TonClientBuilder::default()
        .disable_retry()
        .await
        .unwrap();

    client.ready().await.unwrap();

    client
}
#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt::init();

    let client = client().await;

    let masterchain_info = client.get_masterchain_info().await?;
    tracing::info!(masterchain_info = ?masterchain_info);

    let shards = client.get_shards(masterchain_info.last.seqno).await?;

    tracing::info!(shards = ?shards);

    Ok(())
}
