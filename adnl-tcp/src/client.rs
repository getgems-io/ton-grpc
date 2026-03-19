use crate::aes_ctr::AesCtr;
use crate::codec::PacketCodec;
use crate::connection::Connection;
use crate::key::{Ed25519Key, Ed25519KeyId};
use anyhow::{anyhow, bail};
use ed25519_dalek::VerifyingKey;
use futures::StreamExt;
use std::time::Duration;
use tokio::io::AsyncWriteExt;
use tokio::net::{TcpStream, ToSocketAddrs};
use tokio::time::timeout;
use tokio_util::codec::Framed;

pub type ServerKey = [u8; 32];

pub struct Client;

impl Client {
    pub async fn connect<A: ToSocketAddrs>(
        addr: A,
        server_key: ServerKey,
    ) -> anyhow::Result<Connection> {
        let mut stream = TcpStream::connect(addr).await?;

        let aes_ctr = AesCtr::generate();
        let server_public_key = VerifyingKey::from_bytes(&server_key)?;
        let server_key_id = Ed25519KeyId::from_public_key_bytes(&server_key);
        let client_key = Ed25519Key::generate();

        let (basis, checksum) =
            aes_ctr.encrypt(client_key.expanded_secret_key(), &server_public_key);

        stream.write_all(server_key_id.as_slice()).await?;
        stream.write_all(client_key.public_key().as_bytes()).await?;
        stream.write_all(checksum.as_slice()).await?;
        stream.write_all(basis.as_slice()).await?;
        stream.flush().await?;

        let codec = PacketCodec::from_aes_ctr_as_client(aes_ctr);
        let mut framed = Framed::new(stream, codec);

        let packet = timeout(Duration::from_secs(5), framed.next())
            .await?
            .ok_or(anyhow!("missed empty packet"))??;

        tracing::info!(packet = ?packet, "received packet");
        if packet.is_empty() {
            tracing::info!("handshake ok");
        } else {
            bail!("empty packet expected")
        }

        Ok(Connection::new(framed))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ping::{is_pong_packet, ping_packet};
    use futures::SinkExt;
    use testcontainers_ton::LocalLiteServer;
    use tracing_test::traced_test;

    #[traced_test]
    #[tokio::test]
    #[ignore]
    async fn client_connect() -> anyhow::Result<()> {
        let server = LocalLiteServer::new().await?;

        let _ = Client::connect(server.get_addr(), server.get_server_key()).await?;

        Ok(())
    }

    #[traced_test]
    #[tokio::test]
    #[ignore]
    async fn client_connect_wrong_key() -> anyhow::Result<()> {
        let server = LocalLiteServer::new().await?;
        let mut invalid_key: ServerKey = server.get_server_key();
        invalid_key[0] = invalid_key[0] ^ 1;

        let client = Client::connect(server.get_addr(), invalid_key).await;

        assert!(client.is_err());
        assert_eq!(
            client.err().unwrap().to_string(),
            "missed empty packet".to_string()
        );

        Ok(())
    }

    #[traced_test]
    #[tokio::test]
    #[ignore]
    async fn client_ping() -> anyhow::Result<()> {
        let server = LocalLiteServer::new().await?;
        let mut client = Client::connect(server.get_addr(), server.get_server_key()).await?;

        let sent = client.send(ping_packet()).await;
        let received = client.next().await.unwrap()?;

        assert!(sent.is_ok());
        assert!(is_pong_packet(&received));

        Ok(())
    }
}
