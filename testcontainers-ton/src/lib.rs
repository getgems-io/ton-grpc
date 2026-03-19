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

pub struct LocalLiteServer {
    #[allow(unused)]
    genesis: ContainerAsync<Genesis>,
    #[allow(unused)]
    liteserver: ContainerAsync<LiteServer>,
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
        let liteserver = LiteServer::new(config).start().await?;
        let port = liteserver.get_host_port_ipv4(30004).await?;

        let server_key: ServerKey = base64::engine::general_purpose::STANDARD
            .decode("Wha42OjSNvDaHOjhZhUZu0zW/+wu/+PaltND/a0FbuI=")?
            .as_slice()
            .try_into()?;

        Ok(Self {
            genesis,
            liteserver,
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
}
