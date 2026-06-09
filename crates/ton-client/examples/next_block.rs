use ton_client::Client;
use ton_config::{default_ton_config_url, load_ton_config};
use tonlibjson_client::{MakeTonlibjsonAdapter, TonlibjsonAdapter};
use tower::ServiceExt;

async fn client() -> Client<TonlibjsonAdapter> {
    let config = load_ton_config(default_ton_config_url()).await.unwrap();
    let adapter = MakeTonlibjsonAdapter.oneshot(config).await.unwrap();
    let mut client = Client::new(adapter);

    client.wait_ready().await.unwrap();

    client
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt::init();

    let mut client = client().await;

    let mut current_block = client.get_masterchain_info().await?.last;
    tracing::info!(current_seqno = current_block.seqno);

    for _ in 0..100 {
        current_block = client
            .look_up_block_by_seqno(
                current_block.workchain,
                current_block.shard,
                current_block.seqno + 1,
            )
            .await?;

        tracing::info!(current_block = ?current_block);
    }

    Ok(())
}
