pub mod genesis;
pub mod lite_server;

use crate::genesis::Genesis;
use crate::lite_server::LiteServer;
use anyhow::Result;
use base64::Engine;
use std::net::{Ipv4Addr, SocketAddrV4};
use testcontainers::ContainerAsync;
use testcontainers::runners::AsyncRunner;

pub type ServerKey = [u8; 32];

const LOCALHOST_I32: i32 = 2130706433; // 127.0.0.1 as signed i32
const SERVER_KEY_B64: &str = "Wha42OjSNvDaHOjhZhUZu0zW/+wu/+PaltND/a0FbuI=";

pub struct LocalLiteServer {
    #[allow(unused)]
    genesis: ContainerAsync<Genesis>,
    #[allow(unused)]
    liteserver: ContainerAsync<LiteServer>,
    config_json: serde_json::Value,
    server_key: ServerKey,
    addr: SocketAddrV4,
}

impl LocalLiteServer {
    pub async fn new() -> Result<Self> {
        let genesis = Genesis::default().start().await?;

        let mut config = vec![];
        genesis
            .copy_file_from("/usr/share/data/global.config.json", &mut config)
            .await?;
        let liteserver = LiteServer::new(config.clone()).start().await?;
        let port = liteserver.get_host_port_ipv4(30004).await?;

        let server_key: ServerKey = base64::engine::general_purpose::STANDARD
            .decode(SERVER_KEY_B64)?
            .as_slice()
            .try_into()?;

        let mut config_json: serde_json::Value = serde_json::from_slice(&config)?;
        if let Some(liteservers) = config_json
            .get_mut("liteservers")
            .and_then(|v| v.as_array_mut())
        {
            for ls in liteservers.iter_mut() {
                ls["ip"] = serde_json::json!(LOCALHOST_I32);
                ls["port"] = serde_json::json!(port);
                if let Some(id) = ls.get_mut("id") {
                    id["key"] = serde_json::json!(SERVER_KEY_B64);
                }
            }
        }

        Ok(Self {
            genesis,
            liteserver,
            config_json,
            server_key,
            addr: SocketAddrV4::new(Ipv4Addr::new(127, 0, 0, 1), port),
        })
    }

    pub fn get_server_key(&self) -> ServerKey {
        self.server_key
    }

    pub fn get_addr(&self) -> SocketAddrV4 {
        self.addr
    }

    pub fn get_config_json(&self) -> &serde_json::Value {
        &self.config_json
    }
}
