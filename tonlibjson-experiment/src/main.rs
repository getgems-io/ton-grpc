use futures::TryStreamExt;
use futures::{stream, StreamExt};
use tokio::time::Instant;
use tonlibjson_tokio::{ShortTxId, Ton};

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let ton = Ton::from_config("./liteserver_config.json").await?;

    println!("ton built");

    let now = Instant::now();

    let master = ton.get_masterchain_info().await?;
    let _stream = stream::iter((master.last.seqno - 10000..master.last.seqno).into_iter())
        .for_each_concurrent(500, |seqno| {
            let ton = ton.clone();
            async move {
                match ton.get_shards(seqno).await {
                    Ok(shards) => {
                        if let Some(block) = shards.shards.first() {
                            match ton.get_tx_stream(block.clone()).await.try_collect::<Vec<ShortTxId>>().await {
                                Ok(txs) => {
                                    for tx in txs {
                                        let address = format!("{}:{}", block.workchain, base64_to_hex(&tx.account).unwrap());
                                        match ton.get_account_state(&address).await {
                                            Ok(account) => println!("{}: {}", &address, account["balance"].as_str().unwrap()),
                                            Err(e) => println!("{:?}", e)
                                        }
                                    }
                                },
                                Err(e) => println!("{:?}", e)
                            }
                        } else {
                            println!("no block")
                        }
                    },
                    Err(e) => println!("{:?}", e)
                }
            }
        }).await;

    println!("{:?}", (Instant::now() - now).as_secs());

    Ok(())
}

fn base64_to_hex(b: &str) -> anyhow::Result<String> {
    let bytes = base64::decode(b)?;
    let hex = hex::encode(bytes);

    Ok(hex)
}