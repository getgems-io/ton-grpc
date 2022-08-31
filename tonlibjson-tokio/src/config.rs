use std::str::FromStr;
use reqwest::Url;
use serde::Deserialize;
use config::Config;
use tracing::error;

#[derive(Debug, Deserialize)]
pub struct AppConfig {
    #[serde(default = "ton_default_url")]
    pub config_url: Url
}

fn ton_default_url() -> Url {
    Url::from_str("https://ton.org/global-config.json").unwrap()
}

impl AppConfig {
    pub fn from_env() -> Self {
        Config::builder()
            .add_source(config::Environment::with_prefix("TON").try_parsing(true))
            .build()
            .and_then(|c| c.try_deserialize())
            .map_err(|e| {
                error!("Config error {:?}, start with default config", e); e
            })
            .unwrap()
    }
}
