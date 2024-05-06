use anyhow::bail;
use ed25519_dalek::VerifyingKey;
use futures::SinkExt;
use tokio::io::AsyncReadExt;
use tokio::net::TcpStream;
use tokio_util::codec::Framed;
use crate::aes_ctr::AesCtr;
use crate::codec::PacketCodec;
use crate::connection::Connection;
use crate::key::{Ed25519Key, Ed25519KeyId};
use crate::packet::Packet;

pub struct Server;

impl Server {
    pub async fn handshake(mut stream: TcpStream, server_key: &Ed25519Key) -> anyhow::Result<(VerifyingKey, Connection)> {
        let mut handshake_packet= [0u8; 32 + 32 + 32 + 160];
        let len = stream.read_exact(&mut handshake_packet).await?;
        tracing::info!(len = len, handshake_packet = ?handshake_packet);

        let server_key_id = Ed25519KeyId::from_slice(&handshake_packet[0 .. 32]);
        if server_key_id != *server_key.id() {
            bail!("wrong server key id");
        }

        let client_public_key = VerifyingKey::from_bytes(handshake_packet[32 .. 64].try_into()?)?;
        let checksum: &[u8; 32] = &handshake_packet[64 .. 96].try_into()?;
        let aes_basis_encrypted: &[u8; 160] = &handshake_packet[96 .. 256].try_into()?;

        let aes_ctr = AesCtr::from_encrypted(aes_basis_encrypted, checksum, server_key.expanded_secret_key(), &client_public_key)?;
        let codec = PacketCodec::from_aes_ctr_as_server(aes_ctr);
        let mut inner = Framed::new(stream, codec);

        inner.send(Packet::empty()).await?;

        Ok((client_public_key, Connection::new(inner)))
    }
}

#[cfg(test)]
mod tests {
    use std::sync::OnceLock;
    use tokio::net::TcpListener;
    use crate::client::Client;
    use super::*;

    #[tokio::test]
    #[tracing_test::traced_test]
    async fn handshake_accept_connection() {
        let key = provided_server_key();
        let server_public_key = key.public_key().as_bytes();
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let port = listener.local_addr().unwrap().port();

        tokio::spawn(async move {
            let (stream, _) = listener.accept().await.unwrap();
            let (_client_public_key, _connection) = Server::handshake(stream, key).await.unwrap();
        });
        let connected = Client::connect(format!("127.0.0.1:{}", port), server_public_key).await;

        assert!(connected.is_ok());
    }

    static SERVER_KEY: OnceLock<Ed25519Key> = OnceLock::new();

    fn provided_server_key() -> &'static Ed25519Key {
        SERVER_KEY.get_or_init(|| { Ed25519Key::generate() })
    }
}
