use anyhow::anyhow;
use base64::Engine;
use base64::engine::general_purpose::STANDARD as base64_standard;
use toner::tlb::BoC;

// Successful liteServer.sendMessage response status set by the validator:
// see https://github.com/ton-blockchain/ton `validator/impl/liteserver.cpp` `perform_sendMessage`.
pub(super) const SEND_MSG_STATUS_OK: i32 = 1;

pub(crate) fn decode_message_body(message: &str) -> anyhow::Result<Vec<u8>> {
    base64_standard
        .decode(message)
        .map_err(|e| anyhow!("invalid base64 message body: {}", e))
}

// Returns the standard TON representation hash of the message (root cell hash),
// base64-encoded. Matches `tonlib raw.sendMessageReturnHash` semantics.
pub(super) fn compute_message_hash(body: &[u8]) -> anyhow::Result<String> {
    let boc = BoC::deserialize(body).map_err(|e| anyhow!("invalid message BoC: {}", e))?;
    let root = boc
        .single_root()
        .ok_or_else(|| anyhow!("message BoC must have exactly one root cell"))?;
    Ok(base64_standard.encode(root.hash()))
}

#[cfg(test)]
mod tests {
    use super::*;

    // Well-formed external message BoC reused from
    // `crates/tonlibjson-sys/src/tonemulator.rs` (`tvm_send_external_message_test`).
    // We only need a parseable BoC here; signature/seqno are irrelevant for hash computation.
    const EXTERNAL_MESSAGE_B64: &str = concat!(
        "te6cckEBAwEAiwABmU9Ixy590w1KbQEhtnM/bc6Z4R37unJhdZ5qL+c4gcOgXgUI",
        "RouixUgkDX5KjSTMO1N5Lyyry8pPJ9mrYFqJyQIAAAABZAnLkpmlwn3AAQEE0AIC",
        "AGhiAGihZ5e1vhbvvT4MiEuZcPvPZy8sh4bGgqvHe4vMyoD5odzWUAAAAAAAAAAA",
        "AAAAAAAAyNE/vw=="
    );

    #[test]
    fn decode_message_body_accepts_valid_base64() {
        let bytes = decode_message_body(EXTERNAL_MESSAGE_B64).unwrap();

        assert!(!bytes.is_empty());
    }

    #[test]
    fn decode_message_body_rejects_invalid_base64() {
        let result = decode_message_body("not_base64!@#$");

        assert!(result.is_err());
    }

    #[test]
    fn compute_message_hash_returns_base64_of_root_cell_hash() {
        let body = decode_message_body(EXTERNAL_MESSAGE_B64).unwrap();

        let hash = compute_message_hash(&body).unwrap();

        let decoded = base64_standard.decode(&hash).unwrap();
        assert_eq!(decoded.len(), 32);
    }

    #[test]
    fn compute_message_hash_matches_known_value() {
        let body = decode_message_body(EXTERNAL_MESSAGE_B64).unwrap();

        let hash = compute_message_hash(&body).unwrap();

        assert_eq!(hash, "XKIL+vLR2pBzs9FctHsq/r8Ua0gdoU3c/THAvJpvY+k=");
    }

    #[test]
    fn compute_message_hash_is_deterministic() {
        let body = decode_message_body(EXTERNAL_MESSAGE_B64).unwrap();

        let hash1 = compute_message_hash(&body).unwrap();
        let hash2 = compute_message_hash(&body).unwrap();

        assert_eq!(hash1, hash2);
    }

    #[test]
    fn compute_message_hash_rejects_invalid_boc() {
        let result = compute_message_hash(&[0xde, 0xad, 0xbe, 0xef]);

        assert!(result.is_err());
    }
}

#[cfg(test)]
mod integration {
    use super::*;
    use crate::adapter::LiteServerAdapter;
    use crate::client::LiteServerClient;
    use testcontainers_ton::LocalLiteServer;
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

    async fn setup() -> anyhow::Result<(LiteServerAdapter, LocalLiteServer)> {
        let server = LocalLiteServer::new().await?;
        let client = LiteServerClient::connect(server.addr(), server.server_key()).await?;
        Ok((LiteServerAdapter::new(client), server))
    }
}
