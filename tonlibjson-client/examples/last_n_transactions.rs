use futures::{stream, StreamExt};
use tokio::time::Instant;
use tracing_test::traced_test;
use tonlibjson_client::ton::TonClient;

#[tokio::main]
#[traced_test]
async fn main() -> anyhow::Result<()> {
    let ton = TonClient::from_env().await?;

    let now = Instant::now();

    let master = ton.get_masterchain_info().await?;

    stream::iter(master.last.seqno - 10000..master.last.seqno)
        .for_each_concurrent(500, |seqno| {
            let ton = ton.clone();
            async move {
                match ton.get_shards(seqno).await {
                    Ok(shards) => {
                        if let Some(block) = shards.shards.first() {
                            ton.get_tx_stream(block.clone()).await
                                .for_each_concurrent(10, |tx| async {
                                    let Ok(tx) = tx else {
                                        tracing::error!("{:?}", tx.unwrap_err());

                                        return
                                    };
                                    let address = format!("{}:{}", block.workchain, base64_to_hex(&tx.account).unwrap());
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


fn base64_to_hex(b: &str) -> anyhow::Result<String> {
    let bytes = base64::decode(b)?;
    let hex = hex::encode(bytes);

    Ok(hex)
}
