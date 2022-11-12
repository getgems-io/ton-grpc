use std::str::FromStr;
use reqwest::Url;
use serde::Deserialize;
use config::Config;

#[derive(Debug, Deserialize)]
pub struct AppConfig {
    #[serde(default = "default_ton_config_url")]
    pub config_url: Url,
    #[serde(default = "default_pool_size")]
    pub pool_size: i8
}

fn default_ton_config_url() -> Url {
    Url::from_str("https://ton.org/global-config.json").unwrap()
}
fn default_pool_size() -> i8 { 6 }

impl AppConfig {
    pub fn from_env() -> anyhow::Result<Self> {
        let config = Config::builder()
            .add_source(config::Environment::with_prefix("TON").try_parsing(true))
            .build()
            .and_then(|c| c.try_deserialize())?;

        Ok(config)
    }
}
