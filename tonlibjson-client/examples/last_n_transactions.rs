use futures::{stream, StreamExt};
use tokio::time::Instant;
use tonlibjson_client::ton::TonClientBuilder;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt::init();

    let mut ton = TonClientBuilder::default().await?;
    ton.ready().await?;

    let master = ton.get_masterchain_info().await?;

    let now = Instant::now();

    stream::iter(master.last.seqno - 25000..master.last.seqno)
        .for_each_concurrent(500, |seqno| {
            let ton = ton.clone();
            async move {
                match ton.get_shards(seqno).await {
                    Ok(shards) => {
                        if let Some(block) = shards.shards.first() {
                            ton.get_block_tx_stream(block, false)
                                .for_each_concurrent(10, |tx| async {
                                    let Ok(tx) = tx else {
                                        tracing::error!("{:?}", tx.unwrap_err());

                                        return
                                    };

                                    tracing::info!(tx = ?tx);

                                    let address = tx.account.into_internal(block.workchain).to_string();
                                    match ton.get_account_state(&address).await {
                                        Ok(account) => tracing::info!("{}: {}", &address, account["balance"].as_str().unwrap()),
                                        Err(e) => tracing::error!("{:?}", e)
                                    }
                                }).await;
                        } else {
                            tracing::error!("no block")
                        }
                    },
                    Err(e) => tracing::error!("{:?}", e)
                }
            }
        }).await;

    let timing = (Instant::now() - now).as_secs();

    println!("Time: {:?}", timing);

    Ok(())
}
