use std::net::SocketAddrV4;
use adnl_tcp::client::{AdnlTcpClient, ServerKey};
pub struct LiteServerClient {
    inner: AdnlTcpClient,
}

impl LiteServerClient {
    pub async fn connect(addr: SocketAddrV4, server_key: &ServerKey) -> anyhow::Result<Self> {
        let inner = AdnlTcpClient::connect(addr, server_key).await?;

        Ok(Self { inner })
    }
}


