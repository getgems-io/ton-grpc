use std::fmt::format;
use anyhow::anyhow;
use futures::TryStreamExt;
use futures::{stream, StreamExt};
use serde_json::Value;
use std::fs::File;
use std::future;
use std::io::BufReader;
use std::path::Path;
use std::time::Duration;
use tokio::time::Instant;
use tonlibjson_tokio::{AsyncClient, ClientBuilder, ShortTxId, Ton};
use tower::discover::{ServiceList};
use tower::buffer::Buffer;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let discover = ServiceList::new(build_clients("./liteserver_config.json").await?);

    let emwa = tower::load::PeakEwmaDiscover::new(
        discover,
        Duration::from_millis(300),
        Duration::from_secs(10),
        tower::load::CompleteOnResponse::default(),
    );

    let ton = tower::balance::p2c::Balance::new(emwa);
    let ton = Buffer::new(ton, 200000);
    let mut ton = Ton::new(ton);

    println!("ton built");

    let now = Instant::now();

    let master = ton.get_masterchain_info().await?;
    let _stream = stream::iter((master.last.seqno - 10000..master.last.seqno).into_iter())
        .for_each_concurrent(500, |seqno| {
            let mut ton = ton.clone();
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

async fn build_clients<P: AsRef<Path>>(path: P) -> anyhow::Result<Vec<AsyncClient>> {
    let file = File::open(path)?;
    let reader = BufReader::new(file);
    let config: Value = serde_json::from_reader(reader)?;

    let liteservers = config["liteservers"]
        .as_array()
        .ok_or(anyhow!("No liteservers in config"))?;

    let x: Vec<AsyncClient> = stream::iter(liteservers.to_owned())
        .map(move |liteserver| {
            let mut config = config.clone();
            config["liteservers"] = Value::Array(vec![liteserver.to_owned()]);

            config
        })
        .then(|config| async move {
            let config = config.clone();

            async move {
                let client = ClientBuilder::from_json_config(&config)
                    .disable_logging()
                    .build()
                    .await;
                match client {
                    Ok(client) => {
                        let sync = client.synchronize().await;
                        match sync {
                            Ok(_) => Ok(client),
                            Err(e) => Err(e),
                        }
                    }
                    Err(e) => Err(e),
                }
            }
        })
        .buffer_unordered(100)
        .filter(|client| future::ready(client.is_ok()))
        .try_collect()
        .await?;

    return Ok(x);
}


fn base64_to_hex(b: &str) -> anyhow::Result<String> {
    let bytes = base64::decode(b)?;
    let hex = hex::encode(bytes);

    Ok(hex)
}