use std::time::Duration;
use anyhow::{anyhow, bail};
use ed25519_dalek::VerifyingKey;
use futures::StreamExt;
use tokio::net::{TcpStream, ToSocketAddrs};
use tokio_util::codec::Framed;
use tokio::io::AsyncWriteExt;
use tokio::time::timeout;
use crate::aes_ctr::AesCtr;
use crate::codec::PacketCodec;
use crate::key::{Ed25519Key, Ed25519KeyId};
use crate::connection::Connection;

pub type ServerKey = [u8; 32];

pub struct Client;

impl Client {
    pub async fn connect<A: ToSocketAddrs>(addr: A, server_key: &ServerKey) -> anyhow::Result<Connection> {
        let mut stream = TcpStream::connect(addr).await?;

        let aes_ctr = AesCtr::generate();
        let server_public_key = VerifyingKey::from_bytes(&server_key)?;
        let server_key_id = Ed25519KeyId::from_public_key_bytes(&server_key);
        let client_key = Ed25519Key::generate();

        let (basis, checksum) = aes_ctr.encrypt(client_key.expanded_secret_key(), &server_public_key);

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
    use std::net::{Ipv4Addr, SocketAddrV4};
    use base64::Engine;
    use futures::SinkExt;
    use tracing_test::traced_test;
    use crate::ping::{is_pong_packet, ping_packet};
    use super::*;

    #[traced_test]
    #[tokio::test]
    #[ignore]
    async fn client_connect() -> anyhow::Result<()> {
        let _ = provided_client().await?;

        Ok(())
    }

    #[traced_test]
    #[tokio::test]
    #[ignore]
    async fn client_connect_wrong_key() -> anyhow::Result<()> {
        let ip: i32 = -2018147075;
        let ip = Ipv4Addr::from(ip as u32);
        let port = 46529;
        let key: ServerKey = (0..32).collect::<Vec<_>>().try_into().unwrap();

        tracing::info!("Connecting to {}:{} with key {:?}", ip, port, key);

        let client = Client::connect(SocketAddrV4::new(ip, port), &key).await;

        assert!(client.is_err());
        assert_eq!(client.err().unwrap().to_string(), "missed empty packet".to_string());

        Ok(())
    }

    #[traced_test]
    #[tokio::test]
    #[ignore]
    async fn client_ping() -> anyhow::Result<()> {
        let mut client = provided_client().await?;

        let sent = client.send(ping_packet()).await;
        let received = client.next().await.unwrap()?;

        assert!(sent.is_ok());
        assert!(is_pong_packet(&received));

        Ok(())
    }

    async fn provided_client() -> anyhow::Result<Connection> {
        let ip: i32 = -2018147075;
        let ip = Ipv4Addr::from(ip as u32);
        let port = 46529;
        let key: ServerKey = base64::engine::general_purpose::STANDARD.decode("jLO6yoooqUQqg4/1QXflpv2qGCoXmzZCR+bOsYJ2hxw=")?.as_slice().try_into()?;

        tracing::info!("Connecting to {}:{} with key {:?}", ip, port, key);

        let connection = Client::connect(SocketAddrV4::new(ip, port), &key).await?;

        Ok(connection)
    }
}
