pub mod genesis;
pub mod lite_server;

use crate::genesis::Genesis;
use crate::lite_server::LiteServer;
use anyhow::{Result, anyhow};
use base64::Engine;
use std::net::{Ipv4Addr, SocketAddrV4};
use testcontainers::ContainerAsync;
use testcontainers::runners::AsyncRunner;

pub type ServerKey = [u8; 32];

pub struct LocalLiteServer {
    #[allow(unused)]
    genesis: ContainerAsync<Genesis>,
    #[allow(unused)]
    liteserver: ContainerAsync<LiteServer>,
    server_key: ServerKey,
    addr: SocketAddrV4,
    config: String,
}

impl LocalLiteServer {
    pub async fn new() -> Result<Self> {
        let genesis = Genesis::default().start().await?;

        let mut config_bytes = vec![];
        genesis
            .copy_file_from("/usr/share/data/global.config.json", &mut config_bytes)
            .await?;
        let global_config: serde_json::Value = serde_json::from_slice(&config_bytes)?;
        let liteserver = LiteServer::new(config_bytes).start().await?;
        let port = liteserver.get_host_port_ipv4(30004).await?;

        let server_key: ServerKey = base64::engine::general_purpose::STANDARD
            .decode("Wha42OjSNvDaHOjhZhUZu0zW/+wu/+PaltND/a0FbuI=")?
            .as_slice()
            .try_into()?;

        let addr = SocketAddrV4::new(Ipv4Addr::new(127, 0, 0, 1), port);
        let config = Self::build_config(global_config, &addr, &server_key)?;

        Ok(Self {
            genesis,
            liteserver,
            server_key,
            addr,
            config,
        })
    }

    pub fn get_server_key(&self) -> ServerKey {
        self.server_key
    }

    pub fn get_addr(&self) -> SocketAddrV4 {
        self.addr
    }

    pub fn config(&self) -> &str {
        &self.config
    }

    fn build_config(
        mut config: serde_json::Value,
        addr: &SocketAddrV4,
        server_key: &ServerKey,
    ) -> Result<String> {
        let ip: u32 = (*addr.ip()).into();
        let key = base64::engine::general_purpose::STANDARD.encode(server_key);

        let obj = config
            .as_object_mut()
            .ok_or(anyhow!("global config must be a JSON object"))?;
        obj.insert(
            "liteservers".to_string(),
            serde_json::json!([
                {
                    "id": {
                        "@type": "pub.ed25519",
                        "key": key
                    },
                    "ip": ip as i32,
                    "port": addr.port()
                }
            ]),
        );

        Ok(config.to_string())
    }
}
