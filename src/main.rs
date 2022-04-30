use std::sync::{Arc, Mutex};
use std::thread;
use std::time::{Duration, Instant};
use serde_json::json;
use futures::future::join_all;
use crate::tonlib::{AsyncClient, Client};

mod tonlib;

#[tokio::main(worker_threads = 4)]
async fn main() {
    let client = Arc::new(AsyncClient::new());

    let start = Instant::now();

    let futures = (1..1000).map(|_| {
        tokio::spawn({
            let client = client.clone();
            async move {
                let query = json!({
                    "@type": "blocks.getMasterchainInfo"
                });
                let resp = client.execute(query).await;
            }
        })
    });

    join_all(futures).await;

    println!("{}", (Instant::now() - start).as_secs_f64());
}
