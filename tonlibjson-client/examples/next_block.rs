use tonlibjson_client::ton::{TonClient, TonClientBuilder};

async fn client() -> TonClient {
    let mut client = TonClientBuilder::default().disable_retry().build().unwrap();

    client.ready().await.unwrap();

    client
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt::init();

    let client = client().await;

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
