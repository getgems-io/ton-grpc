use reqwest::IntoUrl;
use serde::{Serialize, Deserialize};
use serde_json::Value;

#[derive(Serialize, Deserialize, Debug, Eq, PartialEq, Clone)]
pub struct TonConfig {
    pub liteservers: Vec<Liteserver>,
    #[serde(flatten)]
    pub data: Value
}

impl ToString for TonConfig {
    fn to_string(&self) -> String {
        serde_json::to_string(self).unwrap()
    }
}

impl TonConfig {
    pub fn with_liteserver(&self, liteserver: &Liteserver) -> Self {
        TonConfig { liteservers: vec![liteserver.clone()], data: self.data.clone() }
    }
}

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
    pub fn id(&self) -> String {
        format!("{}:{}", self.id.typ, self.id.key)
    }
}

pub async fn load_ton_config<U: IntoUrl>(url: U) -> anyhow::Result<TonConfig> {
    let config = reqwest::get(url).await?.text().await?;

    let config = serde_json::from_str(config.as_ref())?;

    Ok(config)
}

#[cfg(test)]
mod tests {
    use serde_json::json;
    use crate::ton_config::{load_ton_config, TonConfig};

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
