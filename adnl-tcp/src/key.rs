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

    pub fn from_slice(slice: &[u8]) -> Self {
        let mut id = [0u8; 32];
        id.copy_from_slice(slice);

        Self(id)
    }

    pub fn as_slice(&self) -> &[u8] {
        self.0.as_slice()
    }
}

pub struct Ed25519Key {
    id: Ed25519KeyId,
    pub_key: VerifyingKey,
    exp_key: ExpandedSecretKey
}

impl Ed25519Key {
    pub fn generate() -> Self {
        let private_key = SigningKey::generate(&mut rand::thread_rng());
        let pub_key = private_key.verifying_key();
        let id = Ed25519KeyId::from_public_key_bytes(pub_key.as_bytes());
        let exp_key: ExpandedSecretKey = private_key.as_bytes().into();

        Self { id, pub_key, exp_key }
    }

    pub fn id(&self) -> &Ed25519KeyId {
        &self.id
    }

    pub fn public_key(&self) -> &VerifyingKey {
        &self.pub_key
    }

    pub fn expanded_secret_key(&self) -> &ExpandedSecretKey {
        &self.exp_key
    }
}
