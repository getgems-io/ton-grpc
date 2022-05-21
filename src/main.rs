use std::sync::Arc;
use std::time::{Duration, Instant};
use crate::tonlib::{AccountTransactionId, AsyncClient, ClientBuilder, ShortTxId};
use futures::future::join_all;
use futures::{stream, Stream, StreamExt};

mod tonlib;

#[tokio::main(worker_threads = 4)]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let pool = tonlib::pool::Pool::from_file("./liteserver_config.json").await?;

    tokio::time::sleep(Duration::from_secs(30)).await;

    Ok(())

    //
    // let client = Arc::new(ClientBuilder::from_file("./liteserver_config.json")?
    //     .disable_logging()
    //     .build().await?);
    //
    // let now = Instant::now();
    // let info = client.get_masterchain_info().await;
    // // let txs = get_tx_from_seqno(&client, info.last.seqno, info.last.seqno).await;
    // let tw = stream::iter((info.last.seqno - 10000)..info.last.seqno)
    //     .then(|seqno| {
    //         let client = &client;
    //
    //         async move {
    //             get_tx_from_seqno(client, seqno, info.last.seqno)
    //         }
    //     }).buffer_unordered(1000)
    //     .map(|r| r.unwrap_or(vec!()))
    //     .concat().await;
    //
    // println!("{:?}", tw);
    //
    // // stream::iter((info.last.seqno - 10000)..info.last.seqno)
    // //     .then(|seqno| async {
    // //         let client = client.clone();
    // //
    // //         get_tx_from_seqno(client, seqno)
    // //     }).collect::<Vec<ShortTxId>>();
    //
    // // for seqno in info.last.seqno - 1000 .. info.last.seqno {
    // //     let shards = client.get_shards(seqno, 0).await;
    // //     for shard in shards {
    // //         let txs = client.get_transactions(&shard).await;
    // //         println!("{}, block contains {:#?} transactions", seqno, txs.len());
    // //     }
    // // }
    //
    // println!("{}", (Instant::now() - now).as_secs_f64());
    //
    // Ok(())
}

async fn get_tx_from_seqno(client: &AsyncClient, seqno: u64, max: u64) -> anyhow::Result<Vec<ShortTxId>> {
    let shards = client.get_shards(seqno, 0).await?;
    let txs = stream::iter(shards)
        .flat_map(|shard| client.get_tx_stream(shard));

   let txs = txs.collect::<Vec<ShortTxId>>().await;

    println!("{}/{}, block contains {:#?} transactions", seqno, max, txs.len());


    return Ok(txs);
}
