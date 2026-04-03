pub mod genesis;
pub mod lite_server;

use crate::genesis::Genesis;
use crate::lite_server::LiteServer as LiteServerImage;
use anyhow::Result;
use base64::Engine;
use std::net::{Ipv4Addr, SocketAddrV4};
use testcontainers::ContainerAsync;
use testcontainers::runners::AsyncRunner;
use ton_config::{LiteServer, LiteServerId, TonConfig};

pub type ServerKey = [u8; 32];

pub struct LocalLiteServer {
    #[allow(unused)]
    genesis: ContainerAsync<Genesis>,
    #[allow(unused)]
    liteserver: ContainerAsync<LiteServerImage>,
    server_key: ServerKey,
    addr: SocketAddrV4,
    config: TonConfig,
}

impl LocalLiteServer {
    pub async fn new() -> Result<Self> {
        let genesis = Genesis::default().start().await?;

        let mut config_bytes = vec![];
        genesis
            .copy_file_from("/usr/share/data/global.config.json", &mut config_bytes)
            .await?;
        let config = TonConfig::try_from(config_bytes.as_slice())?;
        let liteserver = LiteServerImage::new(config_bytes).start().await?;
        let port = liteserver.get_host_port_ipv4(30004).await?;

        let server_key: ServerKey = base64::engine::general_purpose::STANDARD
            .decode("Wha42OjSNvDaHOjhZhUZu0zW/+wu/+PaltND/a0FbuI=")?
            .as_slice()
            .try_into()?;

        let addr = SocketAddrV4::new(Ipv4Addr::new(127, 0, 0, 1), port);
        let key = base64::engine::general_purpose::STANDARD.encode(server_key);

        Ok(Self {
            genesis,
            liteserver,
            server_key,
            addr,
            config: config.with_liteserver(LiteServer::new(LiteServerId { key }, addr)),
        })
    }

    pub fn server_key(&self) -> ServerKey {
        self.server_key
    }

    pub fn addr(&self) -> SocketAddrV4 {
        self.addr
    }

    pub fn config(&self) -> &TonConfig {
        &self.config
    }
}
