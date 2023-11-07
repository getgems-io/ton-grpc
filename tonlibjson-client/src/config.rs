use std::path::PathBuf;
use std::str::FromStr;
use reqwest::Url;
use serde::Deserialize;
use config::Config;

#[derive(Debug, Deserialize)]
pub struct AppConfig {
    #[serde(default = "default_ton_config_url")]
    pub config_url: Url,

    pub config_path: Option<PathBuf>
}

#[cfg(not(feature = "testnet"))]
pub(crate) fn default_ton_config_url() -> Url {
    Url::from_str("https://raw.githubusercontent.com/ton-blockchain/ton-blockchain.github.io/main/global.config.json").unwrap()
}

#[cfg(feature = "testnet")]
pub(crate) fn default_ton_config_url() -> Url {
    Url::from_str("https://raw.githubusercontent.com/ton-blockchain/ton-blockchain.github.io/main/testnet-global.config.json").unwrap()
}

impl AppConfig {
    pub fn from_env() -> anyhow::Result<Self> {
        let config: AppConfig = Config::builder()
            .add_source(config::Environment::with_prefix("TON").try_parsing(true))
            .build()
            .and_then(|c| c.try_deserialize())?;

        Ok(config)
    }
}
