use tracing::info;
use tonlibjson_client::ton::TonClient;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt::init();

    let ton = TonClient::from_env().await?;

    let mainchain_info = ton.get_masterchain_info().await?;

    let mainchain_shards = ton.get_shards(
        mainchain_info.last.workchain,
        mainchain_info.last.shard,
        mainchain_info.last.seqno
    ).await?;

    for shard in mainchain_shards.shards {
        let shards = ton.get_shards(
            shard.workchain,
            shard.shard,
            shard.seqno
        ).await?;

        for shard in shards.shards {
            info!(shard = ?shard, "shard")
        }
    }

    Ok(())
}
