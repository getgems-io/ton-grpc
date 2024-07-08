use reqwest::IntoUrl;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::fmt::{Display, Formatter};
use std::path::Path;

#[derive(Serialize, Deserialize, Debug, Eq, PartialEq, Clone)]
pub struct TonConfig {
    pub liteservers: Vec<LiteServer>,
    #[serde(flatten)]
    pub data: Value,
}

impl TonConfig {
    pub fn with_liteserver(&self, liteserver: &LiteServer) -> Self {
        TonConfig {
            liteservers: vec![liteserver.clone()],
            data: self.data.clone(),
        }
    }
}

impl Display for TonConfig {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "{}",
            serde_json::to_string(self).map_err(|_| std::fmt::Error)?
        )
    }
}

#[derive(Deserialize, Serialize, Hash, Eq, PartialEq, Clone, Debug)]
pub struct LiteServerId {
    #[serde(rename = "@type")]
    pub r#type: String,
    pub key: String,
}

#[derive(Deserialize, Serialize, Hash, Eq, PartialEq, Clone, Debug)]
pub struct LiteServer {
    pub id: LiteServerId,
    pub ip: Option<i32>,
    pub host: Option<String>,
    pub port: u16,
}

impl LiteServer {
    pub fn id(&self) -> String {
        format!("{}:{}", self.id.r#type, self.id.key)
    }

    pub fn with_ip(&self, ip: i32) -> Self {
        LiteServer {
            id: self.id.clone(),
            ip: Some(ip),
            host: self.host.clone(),
            port: self.port,
        }
    }
}

pub async fn load_ton_config(url: impl IntoUrl) -> anyhow::Result<TonConfig> {
    let config = reqwest::get(url).await?.text().await?;
    let config = serde_json::from_str(config.as_ref())?;

    Ok(config)
}

pub async fn read_ton_config(path: impl AsRef<Path>) -> anyhow::Result<TonConfig> {
    let config = tokio::fs::read_to_string(path).await?;
    let config = serde_json::from_str(config.as_ref())?;

    Ok(config)
}


#[cfg(test)]
mod tests {
    use serde_json::{json, Value};
    use crate::discover::config::{load_ton_config, TonConfig};

    #[test]
    fn ton_config_to_string() {
        let input = TonConfig { liteservers: vec![], data: Value::Null };

        let actual = input.to_string();

        assert_eq!("{\"liteservers\":[]}", actual)
    }

    #[tokio::test]
    async fn load_config_mainnet() {
        let url = "https://ton.org/global-config.json";

        let config = load_ton_config(url).await.unwrap();

        assert_eq!(config.data.get("@type").unwrap(), "config.global");
    }

    #[test]
    fn config_equals() {
        let config_lhs = serde_json::from_value::<TonConfig>(json!({
            "@type": "config.global",
            "liteservers": [],
            "dht": {
                "a": 3,
                "k": 3,
            }
        })).unwrap();
        let config_rhs = TonConfig {
            liteservers: vec![],
            data: json!({
                "@type": "config.global",
                "dht": {
                    "a": 3,
                    "k": 3,
                }
            })
        };

        assert_eq!(config_lhs, config_rhs);
    }
}
