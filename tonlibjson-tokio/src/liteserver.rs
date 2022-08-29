use anyhow::anyhow;
use serde::{Deserialize, Serialize};
use std::collections::HashSet;
use std::future::Future;
use std::pin::Pin;
use std::task::{Context, Poll};
use reqwest::{IntoUrl, Url};
use serde_json::{json, Value};
use tokio::sync::mpsc::{channel, Receiver};
use tower::discover::{Change, Discover};
use tower::MakeService;
use crate::{ClientBuilder, ServiceError, TonNaive};
use tower::service_fn;
use tower::util::ServiceFn;

#[derive(Deserialize, Serialize, Hash, Eq, PartialEq, Clone, Debug)]
pub struct LiteserverId {
    #[serde(rename = "@type")]
    typ: String,
    key: String,
}

#[derive(Deserialize, Serialize, Hash, Eq, PartialEq, Clone, Debug)]
pub struct Liteserver {
    id: LiteserverId,
    ip: i32,
    port: u16,
}

impl Liteserver {
    pub fn identifier(&self) -> String {
        format!("{}:{}", self.id.typ, self.id.key)
    }
}

pub struct LiteserverConfig {
    config: Value,
    liteserver: Liteserver
}

pub async fn load_ton_config<U: IntoUrl>(url: U) -> anyhow::Result<String> {
    let config = reqwest::get(url).await?.text().await?;

    Ok(config)
}

pub fn extract_liteserver_list(config: &str) -> anyhow::Result<HashSet<Liteserver>> {
    let decoded_json = serde_json::from_str::<Value>(config)?;

    let liteservers = decoded_json
        .get("liteservers")
        .ok_or_else(|| anyhow!("liteservers not found"))?
        .as_array()
        .ok_or_else(|| anyhow!("liteservers is not array"))?
        .iter()
        .map(|v| serde_json::from_value::<Liteserver>(v.clone()))
        .collect::<Result<Vec<Liteserver>, serde_json::Error>>()?;

    let liteservers = HashSet::from_iter(liteservers);

    Ok(liteservers)
}

#[cfg(test)]
mod tests {
    use crate::liteserver::{Liteserver, LiteserverId, load_ton_config};
    use crate::liteserver::extract_liteserver_list;
    use serde_json::json;

    #[tokio::test]
    async fn load_config_mainnet() {
        let url = "https://ton.org/global-config.json";

        let config = load_ton_config(url).await.unwrap();

        assert!(config.contains("\"@type\": \"config.global\""))
    }

    #[test]
    fn extract_liteserver_list_from_config() {
        let config = json!({"liteservers": [
            {
              "ip": 84478511,
              "port": 19949,
              "id": {
                "@type": "pub.ed25519",
                "key": "n4VDnSCUuSpjnCyUk9e3QOOd6o0ItSWYbTnW3Wnn8wk="
              }
            },
            {
              "ip": 84478479,
              "port": 48014,
              "id": {
                "@type": "pub.ed25519",
                "key": "3XO67K/qi+gu3T9v8G2hx1yNmWZhccL3O7SoosFo8G0="
              }
            }
        ]}).to_string();

        let liteservers = extract_liteserver_list(&config).unwrap();

        assert_eq!(liteservers.len(), 2);
        assert!(liteservers.contains(&Liteserver {
            ip: 84478511,
            port: 19949,
            id: LiteserverId {
                typ: "pub.ed25519".to_string(),
                key: "n4VDnSCUuSpjnCyUk9e3QOOd6o0ItSWYbTnW3Wnn8wk=".to_string()
            }
        }))
    }
}
