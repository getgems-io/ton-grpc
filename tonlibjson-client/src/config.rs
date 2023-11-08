use std::str::FromStr;
use reqwest::Url;

#[cfg(not(feature = "testnet"))]
pub(crate) fn default_ton_config_url() -> Url {
    Url::from_str("https://raw.githubusercontent.com/ton-blockchain/ton-blockchain.github.io/main/global.config.json").unwrap()
}

#[cfg(feature = "testnet")]
pub(crate) fn default_ton_config_url() -> Url {
    Url::from_str("https://raw.githubusercontent.com/ton-blockchain/ton-blockchain.github.io/main/testnet-global.config.json").unwrap()
}
