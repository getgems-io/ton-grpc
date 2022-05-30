use anyhow::anyhow;
use futures::{stream, Stream, StreamExt};
use serde_json::Value;
use std::fs::File;
use std::future;
use std::io::BufReader;
use std::iter::repeat;
use std::path::Path;
use std::sync::Arc;
use std::time::Duration;
use tonlibjson_tokio::{AsyncClient, ClientBuilder, GetMasterchainInfo, MasterchainInfo};
use tower::discover::{Change, Discover, ServiceList};
use tower::{Service, ServiceExt};
use futures::TryStreamExt;
use tokio::time::Instant;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let discover = ServiceList::new(build_clients("./liteserver_config.json").await?);

    let emwa = tower::load::PeakEwmaDiscover::new(
        discover,
        Duration::from_millis(300),
        Duration::from_secs(10),
        tower::load::CompleteOnResponse::default(),
    );

    let mut ton = tower::balance::p2c::Balance::new(emwa);

    let request = serde_json::to_value(GetMasterchainInfo {})?;

    println!("ton built");

    let now = Instant::now();

    let mut responses = ton
        .ready()
        .await
        .map_err(|e| anyhow!(e))?
        .call_all(stream::iter(repeat(request).take(100000)))
        .unordered();

    while let Ok(Some(rsp)) = responses.try_next().await {
        println!("{:?}", rsp);
    }

    println!("{:?}", (Instant::now() - now).as_secs());

    Ok(())
}

async fn build_clients<P: AsRef<Path>>(
    path: P,
) -> anyhow::Result<Vec<AsyncClient>> {
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
                let client = ClientBuilder::from_json_config(&config).disable_logging().build().await;
                match client {
                    Ok(client) => {
                        let sync = client.synchronize().await;
                        match sync {
                            Ok(_) => Ok(client),
                            Err(e) => Err(e)
                        }
                    },
                    Err(e) => Err(e)
                }
            }
        })
        .buffer_unordered(100)
        .filter(|client| future::ready(client.is_ok()))
        .try_collect().await?;

    return Ok(x);
}
