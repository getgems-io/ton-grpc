use futures::{StreamExt, stream};
use tokio::time::Instant;
use ton_client::Client;
use ton_config::{default_ton_config_url, load_ton_config};
use tonlibjson_client::MakeTonlibjsonAdapter;
use tower::ServiceExt;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt::init();

    let config = load_ton_config(default_ton_config_url()).await?;
    let adapter = MakeTonlibjsonAdapter.oneshot(config).await?;
    let mut ton = Client::new(adapter);
    ton.wait_ready().await?;

    let master = ton.get_masterchain_info().await?;

    let now = Instant::now();

    stream::iter(master.last.seqno - 25000..master.last.seqno)
        .for_each_concurrent(500, |seqno| {
            let mut ton = ton.clone();
            async move {
                match ton.get_shards(seqno).await {
                    Ok(shards) => {
                        if let Some(block) = shards.shards.first() {
                            ton.get_block_tx_id_stream(block, false)
                                .for_each_concurrent(10, |tx| {
                                    let mut ton = ton.clone();
                                    async move {
                                        let Ok(tx) = tx else {
                                            tracing::error!("{:?}", tx.unwrap_err());

                                            return;
                                        };

                                        tracing::info!(tx = ?tx);

                                        match ton.get_account_state(&tx.account).await {
                                            Ok(account) => {
                                                tracing::info!(
                                                    "{}: {:?}",
                                                    &tx.account,
                                                    account.balance
                                                )
                                            }
                                            Err(e) => tracing::error!("{:?}", e),
                                        }
                                    }
                                })
                                .await;
                        } else {
                            tracing::error!("no block")
                        }
                    }
                    Err(e) => tracing::error!("{:?}", e),
                }
            }
        })
        .await;

    let timing = (Instant::now() - now).as_secs();

    println!("Time: {timing:?}");

    Ok(())
}
