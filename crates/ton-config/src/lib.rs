use reqwest::IntoUrl;
use serde::{Deserialize, Deserializer, Serialize, Serializer};
use serde_json::Value;
use std::fmt::{Display, Formatter};
use std::net::{Ipv4Addr, SocketAddrV4};
use std::path::Path;
use std::str::FromStr;
use url::Url;

#[derive(Serialize, Deserialize, Debug, Eq, PartialEq, Clone)]
#[serde(tag = "@type")]
#[serde(rename = "config.global")]
pub struct TonConfig {
    pub liteservers: Vec<LiteServer>,
    #[serde(flatten)]
    pub data: Value,
}

impl TonConfig {
    pub fn with_liteserver(&self, liteserver: LiteServer) -> Self {
        TonConfig {
            liteservers: vec![liteserver],
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
#[serde(tag = "@type")]
#[serde(rename = "pub.ed25519")]
pub struct LiteServerId {
    pub key: String,
}

impl Display for LiteServerId {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.key)
    }
}

#[derive(Hash, Eq, PartialEq, Clone, Debug)]
pub struct LiteServer {
    pub id: LiteServerId,
    pub addr: SocketAddrV4,
}

impl LiteServer {
    pub fn new(id: LiteServerId, addr: SocketAddrV4) -> Self {
        Self { id, addr }
    }

    pub fn id(&self) -> String {
        self.id.key.to_string()
    }
}

impl Serialize for LiteServer {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        #[derive(Serialize)]
        struct Wire<'a> {
            id: &'a LiteServerId,
            ip: i32,
            port: u16,
        }

        Wire {
            id: &self.id,
            ip: u32::from(*self.addr.ip()) as i32,
            port: self.addr.port(),
        }
        .serialize(serializer)
    }
}

impl<'de> Deserialize<'de> for LiteServer {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        #[derive(Deserialize)]
        struct Wire {
            id: LiteServerId,
            ip: i32,
            port: u16,
        }

        let wire = Wire::deserialize(deserializer)?;

        Ok(LiteServer {
            id: wire.id,
            addr: SocketAddrV4::new(Ipv4Addr::from(wire.ip as u32), wire.port),
        })
    }
}

#[cfg(not(feature = "testnet"))]
pub fn default_ton_config_url() -> Url {
    Url::from_str("https://raw.githubusercontent.com/ton-blockchain/ton-blockchain.github.io/main/global.config.json").unwrap()
}

#[cfg(feature = "testnet")]
pub fn default_ton_config_url() -> Url {
    Url::from_str("https://raw.githubusercontent.com/ton-blockchain/ton-blockchain.github.io/main/testnet-global.config.json").unwrap()
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
    use crate::{LiteServer, LiteServerId, TonConfig, load_ton_config};
    use serde_json::{Value, json};
    use std::net::{Ipv4Addr, SocketAddrV4};

    #[test]
    fn serialize_to_json_string() {
        let config = TonConfig {
            liteservers: vec![],
            data: Value::Null,
        };

        let result = config.to_string();

        assert_eq!(result, "{\"@type\":\"config.global\",\"liteservers\":[]}");
    }

    #[test]
    fn deserialize_preserves_extra_fields_in_data() {
        let json = json!({
            "@type": "config.global",
            "liteservers": [],
            "dht": { "a": 3, "k": 3 }
        });

        let result = serde_json::from_value::<TonConfig>(json).unwrap();

        assert_eq!(result.data["dht"], json!({ "a": 3, "k": 3 }));
    }

    #[test]
    fn serialize_roundtrip() {
        let json = json!({
            "@type": "config.global",
            "liteservers": [],
            "dht": { "a": 3, "k": 3 }
        });

        let config = serde_json::from_value::<TonConfig>(json).unwrap();
        let result = serde_json::to_value(&config).unwrap();

        assert_eq!(result["@type"], "config.global");
        assert_eq!(result["dht"], json!({ "a": 3, "k": 3 }));
        assert!(result["liteservers"].as_array().unwrap().is_empty());
    }

    #[test]
    fn deserialize_liteserver() {
        let json = json!({
            "id": { "@type": "pub.ed25519", "key": "abc123" },
            "ip": 1137658550,
            "port": 4924
        });

        let result = serde_json::from_value::<LiteServer>(json).unwrap();

        assert_eq!(
            result.addr,
            SocketAddrV4::new(Ipv4Addr::new(67, 207, 74, 182), 4924)
        );
    }

    #[test]
    fn serialize_liteserver_roundtrip() {
        let ls = LiteServer {
            id: LiteServerId { key: "abc".into() },
            addr: SocketAddrV4::new(Ipv4Addr::new(67, 207, 74, 182), 4924),
        };

        let json = serde_json::to_value(&ls).unwrap();

        assert_eq!(json["ip"], 1137658550);
        assert_eq!(json["port"], 4924);
        assert_eq!(json["id"]["@type"], "pub.ed25519");
        assert_eq!(json["id"]["key"], "abc");
    }

    #[tokio::test]
    async fn load_config_mainnet() {
        let url = "https://ton.org/global-config.json";

        let result = load_ton_config(url).await.unwrap();

        assert_eq!(result.data.get("@type").unwrap(), "config.global");
    }
}
