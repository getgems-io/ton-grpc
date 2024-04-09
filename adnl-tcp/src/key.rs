use std::ops::Mul;
use anyhow::bail;
use ed25519_dalek::hazmat::ExpandedSecretKey;
use ed25519_dalek::{SigningKey, VerifyingKey};
use sha2::{Digest, Sha256};

#[derive(Debug, Eq, PartialEq)]
pub struct Ed25519KeyId([u8; 32]);

impl Ed25519KeyId {
    const KEY_TYPE: [u8; 4] = [0xC6, 0xB4, 0x13, 0x48];

    pub fn from_public_key_bytes(public_key: &[u8; 32]) -> Self {
        Self(Sha256::default()
            .chain_update(Self::KEY_TYPE.as_slice())
            .chain_update(public_key.as_slice())
            .finalize()
            .into())
    }

    pub fn as_slice(&self) -> &[u8] {
        self.0.as_slice()
    }
}

pub struct Ed25519Key {
    id: Ed25519KeyId,
    pub_key: VerifyingKey,
    exp_key: Option<ExpandedSecretKey>
}

impl Ed25519Key {
    pub fn from_public_key_bytes(public_key: &[u8; 32]) -> anyhow::Result<Self> {
        let key_id = Ed25519KeyId::from_public_key_bytes(public_key);

        Ok(Self {
            id: key_id,
            pub_key: VerifyingKey::from_bytes(public_key)?,
            exp_key: None
        })
    }

    pub fn generate() -> Self {
        let private_key = SigningKey::generate(&mut rand::thread_rng());
        let public_key = private_key.verifying_key();
        let key_id = Ed25519KeyId::from_public_key_bytes(public_key.as_bytes());
        let exp_key: ExpandedSecretKey = private_key.as_bytes().into();

        Self {
            id: key_id,
            pub_key: public_key,
            exp_key: Some(exp_key)
        }
    }

    pub fn id(&self) -> &Ed25519KeyId {
        &self.id
    }

    pub fn public_key(&self) -> &VerifyingKey {
        &self.pub_key
    }

    pub fn shared_key(&self, other: &Ed25519Key) -> anyhow::Result<[u8; 32]> {
        let Some(exp_key) = self.exp_key.as_ref() else {
            bail!("No expanded secret key");
        };

        Ok(other.pub_key.to_montgomery().mul(exp_key.scalar).to_bytes())
    }
}
