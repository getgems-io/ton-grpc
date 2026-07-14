use crate::tl::{LiteServerSendMessage, LiteServerSendMsgStatus};
use anyhow::{Context, anyhow};
use base64::Engine;
use base64::engine::general_purpose::STANDARD as base64_standard;
use toner::tlb::BoC;

impl LiteServerSendMessage {
    pub fn from_base64<T: AsRef<[u8]>>(message: T) -> anyhow::Result<Self> {
        let body = base64_standard
            .decode(message)
            .context("failed to decode message")?;

        Ok(Self { body })
    }

    pub fn hash(&self) -> anyhow::Result<[u8; 32]> {
        let boc = BoC::deserialize(&self.body)?;
        let root = boc.single_root().ok_or(anyhow!("no root"))?;

        Ok(root.hash())
    }

    pub fn hash_base64(&self) -> anyhow::Result<String> {
        self.hash().map(|hash| base64_standard.encode(hash))
    }
}

impl LiteServerSendMsgStatus {
    const OK: i32 = 1;
    pub fn is_ok(&self) -> bool {
        self.status == Self::OK
    }

    pub fn ensure_ok(&self) -> anyhow::Result<()> {
        if self.is_ok() {
            Ok(())
        } else {
            Err(anyhow!("unexpected message status: {}", self.status))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    const EXTERNAL_MESSAGE_B64: &str = concat!(
        "te6cckEBAwEAiwABmU9Ixy590w1KbQEhtnM/bc6Z4R37unJhdZ5qL+c4gcOgXgUI",
        "RouixUgkDX5KjSTMO1N5Lyyry8pPJ9mrYFqJyQIAAAABZAnLkpmlwn3AAQEE0AIC",
        "AGhiAGihZ5e1vhbvvT4MiEuZcPvPZy8sh4bGgqvHe4vMyoD5odzWUAAAAAAAAAAA",
        "AAAAAAAAyNE/vw=="
    );

    #[test]
    fn decode_message_body_accepts_valid_base64() {
        let msg = LiteServerSendMessage::from_base64(EXTERNAL_MESSAGE_B64).unwrap();

        assert!(!msg.body.is_empty());
    }

    #[test]
    fn decode_message_body_rejects_invalid_base64() {
        let result = LiteServerSendMessage::from_base64("not_base64!@#$");

        assert!(result.is_err());
    }

    #[test]
    fn compute_message_hash_returns_base64_of_root_cell_hash() {
        let req = LiteServerSendMessage::from_base64(EXTERNAL_MESSAGE_B64).unwrap();

        let hash = req.hash_base64().unwrap();

        let decoded = base64_standard.decode(&hash).unwrap();
        assert_eq!(decoded.len(), 32);
    }

    #[test]
    fn compute_message_hash_matches_known_value() {
        let req = LiteServerSendMessage::from_base64(EXTERNAL_MESSAGE_B64).unwrap();

        let hash = req.hash_base64().unwrap();

        assert_eq!(hash, "XKIL+vLR2pBzs9FctHsq/r8Ua0gdoU3c/THAvJpvY+k=");
    }

    #[test]
    fn compute_message_hash_is_deterministic() {
        let req = LiteServerSendMessage::from_base64(EXTERNAL_MESSAGE_B64).unwrap();

        let hash1 = req.hash_base64().unwrap();
        let hash2 = req.hash_base64().unwrap();

        assert_eq!(hash1, hash2);
    }

    #[test]
    fn compute_message_hash_rejects_invalid_boc() {
        let result = LiteServerSendMessage {
            body: vec![0xde, 0xad, 0xbe, 0xef],
        };

        let hash = result.hash_base64();

        assert!(hash.is_err());
    }
}

#[cfg(test)]
mod integration {
    use super::*;
    use crate::adapter::LiteServerAdapter;
    use crate::client::LiteServerClient;
    use testcontainers_ton::{LocalLiteServer, SharedLiteServer};
    use ton_tower::request::SendMessage;
    use tower::ServiceExt;
    use tracing_test::traced_test;

    #[tokio::test]
    #[traced_test]
    async fn send_message_rejects_invalid_base64() -> anyhow::Result<()> {
        let (adapter, _server) = setup().await?;
        let body = "not_base64!@#$".to_string();

        let result = adapter.oneshot(SendMessage { body }).await;

        assert!(result.is_err());
        Ok(())
    }

    #[tokio::test]
    #[traced_test]
    async fn send_message_rejects_invalid_boc_at_validator() -> anyhow::Result<()> {
        let (adapter, _server) = setup().await?;
        let body = base64_standard.encode([0xde, 0xad, 0xbe, 0xef]);

        let result = adapter.oneshot(SendMessage { body }).await;

        assert!(result.is_err());
        Ok(())
    }

    async fn setup() -> anyhow::Result<(LiteServerAdapter, SharedLiteServer)> {
        let server = LocalLiteServer::shared().await?;
        let client = LiteServerClient::connect(server.addr(), server.server_key()).await?;
        Ok((LiteServerAdapter::new(client), server))
    }
}
