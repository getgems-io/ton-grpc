use std::fs::File;
use std::io::BufReader;
use std::path::Path;
use std::iter::Cycle;
use std::sync::Arc;
use std::time::Duration;
use serde_json::{json, Value};
use tonlibjson_rs::Client;
use crate::{AsyncClient, ClientBuilder, stream};
use futures::{StreamExt, TryStreamExt};
use tokio::sync::{Mutex, RwLock};
use tokio::time::Instant;
use crate::tonlib::{MasterchainInfoResponse, TlBlock};

pub struct Pool {
    clients: Vec<AsyncClient>,
    idx: Arc<RwLock<usize>>
}

impl Pool {
    pub async fn from_file<P: AsRef<Path>>(path: P) -> anyhow::Result<Self> {
        let file = File::open(path)?;
        let reader = BufReader::new(file);
        let config: Value = serde_json::from_reader(reader)?;

        let mut clients = Vec::new();
        for lite in config["liteservers"].as_array().unwrap().into_iter() {
            let mut cfg = config.clone();
            cfg["liteservers"] = Value::Array(vec![lite.clone()]);

            let client = ClientBuilder::from_json_config(&cfg)?
                .disable_logging()
                .build()
                .await?;

            clients.push(client);
        }

        let idx = Arc::new(RwLock::new(0));

        Ok(Self { clients, idx })
    }

    pub fn len(&self) -> usize {
        return self.workers.len();
    }

    pub async fn get_masterchain_info(&self) -> MasterchainInfoResponse {
        let client = self.next().await.unwrap().read().await;

        return client.get_masterchain_info();
    }

    async fn next(&self) -> Option<&Arc<RwLock<Worker>>> {
        let mut idx = self.idx.write().await;
        let s = self.workers.get(*idx);

        *idx = (*idx + 1) % self.workers.len();
        s.clone()
    }
}
