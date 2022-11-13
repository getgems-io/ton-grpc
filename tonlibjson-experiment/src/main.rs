use futures::{stream, StreamExt};
use serde_json::Value;
use tokio::time::Instant;
use tower::{BoxError, Service};
use tonlibjson_tokio::Ton;
use tonlibjson_tokio::request::Request;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt::init();

    let ton = Ton::balanced().await?;

    let now = Instant::now();

    let _ = run(ton).await;

    let balanced_timing = (Instant::now() - now).as_secs();

    println!("Time: {:?}", balanced_timing);

    Ok(())
}

async fn run<S>(ton: Ton<S>) -> anyhow::Result<()> where S : Service<Request, Response = Value, Error = BoxError> + Clone {
    let master = ton.get_masterchain_info().await?;

    stream::iter(master.last.seqno - 1000..master.last.seqno)
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

    Ok(())
}

fn base64_to_hex(b: &str) -> anyhow::Result<String> {
    let bytes = base64::decode(b)?;
    let hex = hex::encode(bytes);

    Ok(hex)
}
