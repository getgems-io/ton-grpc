pub mod genesis;
pub mod lite_server;

use crate::genesis::Genesis;
use crate::lite_server::LiteServer as LiteServerImage;
use anyhow::Result;
use base64::Engine;
use std::fs::{File, OpenOptions};
use std::net::{Ipv4Addr, SocketAddrV4};
use testcontainers::runners::AsyncRunner;
use testcontainers::{ContainerAsync, ImageExt, ReuseDirective};
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

/// Read-only handle to the TON containers reused across test processes.
pub struct SharedLiteServer {
    server: LocalLiteServer,
    _exclusive_lock: Option<File>,
}

impl LocalLiteServer {
    pub async fn new() -> Result<Self> {
        let genesis = Genesis::default().start().await?;

        Self::start(genesis, false).await
    }

    /// Starts or reuses one named container pair on the current Docker host.
    ///
    /// The pair persists between test runs. Remove stale containers with:
    /// `docker rm -f testcontainers-ton-genesis-v4-2-0 testcontainers-ton-liteserver-v4-2-0`.
    pub async fn shared() -> Result<SharedLiteServer> {
        let _lock = tokio::task::spawn_blocking(shared_startup_lock).await??;
        let genesis = Genesis::default()
            .with_container_name("testcontainers-ton-genesis-v4-2-0")
            .with_reuse(ReuseDirective::Always)
            .start()
            .await?;

        Ok(SharedLiteServer {
            server: Self::start(genesis, true).await?,
            _exclusive_lock: None,
        })
    }

    /// Reuses the shared pair while preventing concurrent mutating tests.
    ///
    /// The exclusive lease is released when the returned handle is dropped.
    pub async fn shared_exclusive() -> Result<SharedLiteServer> {
        let exclusive_lock = tokio::task::spawn_blocking(shared_exclusive_lock).await??;
        let mut server = Self::shared().await?;
        server._exclusive_lock = Some(exclusive_lock);

        Ok(server)
    }

    async fn start(genesis: ContainerAsync<Genesis>, shared: bool) -> Result<Self> {
        let mut config_bytes = vec![];
        genesis
            .copy_file_from("/usr/share/data/global.config.json", &mut config_bytes)
            .await?;
        let config = TonConfig::try_from(config_bytes.as_slice())?;
        let liteserver = if shared {
            LiteServerImage::new(config_bytes)
                .with_container_name("testcontainers-ton-liteserver-v4-2-0")
                .with_reuse(ReuseDirective::Always)
                .start()
                .await?
        } else {
            LiteServerImage::new(config_bytes).start().await?
        };
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

    pub async fn liteserver_stop(&self) -> Result<()> {
        self.liteserver.stop_with_timeout(Some(0)).await?;

        Ok(())
    }

    pub async fn liteserver_pause(&self) -> Result<()> {
        self.liteserver.pause().await?;

        Ok(())
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

impl SharedLiteServer {
    pub fn server_key(&self) -> ServerKey {
        self.server.server_key()
    }

    pub fn addr(&self) -> SocketAddrV4 {
        self.server.addr()
    }

    pub fn config(&self) -> &TonConfig {
        self.server.config()
    }
}

fn shared_startup_lock() -> Result<File> {
    shared_lock("testcontainers-ton-v4-2-0.lock")
}

fn shared_exclusive_lock() -> Result<File> {
    shared_lock("testcontainers-ton-v4-2-0-exclusive.lock")
}

fn shared_lock(name: &str) -> Result<File> {
    let lock = OpenOptions::new()
        .create(true)
        .read(true)
        .write(true)
        .open(std::env::temp_dir().join(name))?;
    lock.lock()?;

    Ok(lock)
}

#[cfg(test)]
mod integration {
    use super::*;
    use std::time::Duration;

    #[tokio::test]
    async fn shared_should_return_same_server_to_parallel_callers() -> Result<()> {
        let (first, second) = tokio::join!(LocalLiteServer::shared(), LocalLiteServer::shared());

        let first = first?;
        let second = second?;

        assert_eq!(first.addr(), second.addr());
        assert_eq!(first.server_key(), second.server_key());
        Ok(())
    }

    #[tokio::test]
    async fn shared_exclusive_should_serialize_callers() -> Result<()> {
        let first = LocalLiteServer::shared_exclusive().await?;
        let second = LocalLiteServer::shared_exclusive();
        tokio::pin!(second);

        tokio::select! {
            _ = &mut second => panic!("second caller acquired lease early"),
            () = tokio::time::sleep(Duration::from_millis(100)) => {}
        }

        drop(first);

        let second = tokio::time::timeout(Duration::from_secs(5), second).await??;
        assert!(second.addr().ip().is_loopback());
        Ok(())
    }
}
