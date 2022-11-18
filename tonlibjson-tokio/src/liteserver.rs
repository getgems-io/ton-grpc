use anyhow::anyhow;
use serde::{Deserialize, Serialize};
use std::collections::HashSet;
use serde_json::Value;

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

#[derive(Clone, Debug)]
pub struct LiteserverConfig {
    pub config: Value,
    pub liteserver: Liteserver
}

impl LiteserverConfig {
    pub fn new(config: Value, liteserver: Liteserver) -> Self {
        Self {
            config,
            liteserver
        }
    }

    pub fn to_config(&self) -> anyhow::Result<Value> {
        let json = serde_json::to_value(self.liteserver.clone())?;
        let mut config = self.config.clone();

        config["liteservers"] = Value::Array(vec![json]);

        Ok(config)
    }
}

impl Liteserver {
    pub fn identifier(&self) -> String {
        format!("{}:{}", self.id.typ, self.id.key)
    }
}

pub fn extract_liteserver_list(config: &Value) -> anyhow::Result<HashSet<Liteserver>> {
    let liteservers = config
        .get("liteservers")
        .ok_or_else(|| anyhow!("liteservers not found"))?
        .as_array()
        .ok_or_else(|| anyhow!("liteservers is not array"))?
        .iter()
        .map(|v| serde_json::from_value::<Liteserver>(v.to_owned()))
        .collect::<Result<Vec<Liteserver>, serde_json::Error>>()?;

    let liteservers = HashSet::from_iter(liteservers);

    Ok(liteservers)
}

#[cfg(test)]
mod tests {
    use crate::liteserver::{Liteserver, LiteserverId, load_ton_config};
    use crate::liteserver::extract_liteserver_list;
    use serde_json::json;

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
        ]});

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
