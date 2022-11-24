mod retry;
mod discover;
mod make;
mod client;
mod config;
mod ton_config;
pub mod request;
pub mod block;
pub mod session;
pub mod ton;

use std::error::Error;
use std::fmt::{Display, Formatter};
use std::path::Path;
use serde::Deserialize;
use serde_json::{json, Value};
use tower::Service;
use crate::client::Client;
use crate::request::Request;

pub struct ClientBuilder {
    config: Value,
    disable_logging: Option<Value>,
}

impl ClientBuilder {
    pub fn from_config(config: &str) -> Self {
        let full_config = json!({
            "@type": "init",
            "options": {
                "@type": "options",
                "config": {
                    "@type": "config",
                    "config": config,
                    "use_callbacks_for_network": false,
                    "blockchain_name": "",
                    "ignore_cache": true
                },
                "keystore_type": {
                    "@type": "keyStoreTypeInMemory"
                }
            }
        });

        Self {
            config: full_config,
            disable_logging: None,
        }
    }

    pub fn from_file<P: AsRef<Path>>(path: P) -> anyhow::Result<Self> {
        let config = std::fs::read_to_string(&path)?;

        Ok(ClientBuilder::from_config(&config))
    }

    pub fn disable_logging(&mut self) -> &mut Self {
        self.disable_logging = Some(json!({
            "@type": "setLogVerbosityLevel",
            "new_verbosity_level": 1
        }));

        self
    }

    pub async fn build(&self) -> anyhow::Result<Client> {
        #[derive(Deserialize)]
        struct Void {}

        let mut client = Client::new();
        if let Some(ref disable_logging) = self.disable_logging {
            client.call(Request::new(disable_logging.clone())?).await?;
        }

        client.call(Request::new(self.config.clone())?).await?;

        Ok(client)
    }
}

#[derive(Debug, Deserialize)]
pub struct TonError {
    code: i32,
    message: String,
}

impl Display for TonError {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "Ton api error occurred with code {}, message {}",
            self.code, self.message
        )
    }
}

impl Error for TonError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        None
    }
}
