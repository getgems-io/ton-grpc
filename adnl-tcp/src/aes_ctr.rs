use aes::cipher::generic_array::GenericArray;
use aes::cipher::KeyIvInit;
use aes::cipher::StreamCipher;
use anyhow::{anyhow, bail};
use ed25519_dalek::hazmat::ExpandedSecretKey;
use ed25519_dalek::VerifyingKey;
use sha2::{Digest, Sha256};
use std::ops::Mul;

pub type Aes256Ctr128 = ctr::Ctr128BE<aes::Aes256>;

pub struct AesCtr {
    basis: [u8; 160],
}

impl AesCtr {
    pub fn generate() -> Self {
        let mut basis = [0u8; 160];
        rand::fill(&mut basis);

        Self { basis }
    }

    pub fn from_encrypted(
        basis: &[u8; 160],
        checksum: &[u8; 32],
        expanded_secret_key: &ExpandedSecretKey,
        verifying_key: &VerifyingKey,
    ) -> anyhow::Result<Self> {
        let x = verifying_key
            .to_montgomery()
            .mul(expanded_secret_key.scalar)
            .to_bytes();

        let mut cipher = Self::cipher(&x, checksum);
        let mut basis_decrypted = [0u8; 160];
        cipher
            .apply_keystream_b2b(basis, &mut basis_decrypted)
            .map_err(|e| anyhow!(e))?;

        if Sha256::digest(basis_decrypted).as_slice() != checksum {
            bail!("wrong handshake checksum");
        }

        Ok(Self {
            basis: basis_decrypted,
        })
    }

    pub fn into_bytes(self) -> [u8; 160] {
        self.basis
    }

    pub fn encrypt(
        &self,
        expanded_secret_key: &ExpandedSecretKey,
        verifying_key: &VerifyingKey,
    ) -> ([u8; 160], [u8; 32]) {
        let checksum: [u8; 32] = Sha256::digest(self.basis).into();
        let x = verifying_key
            .to_montgomery()
            .mul(expanded_secret_key.scalar)
            .to_bytes();

        let mut cipher = Self::cipher(&x, &checksum);
        let mut basis_encrypted = [0u8; 160];
        cipher
            .apply_keystream_b2b(&self.basis, &mut basis_encrypted)
            .unwrap();

        (basis_encrypted, checksum)
    }

    fn cipher(x: &[u8; 32], y: &[u8; 32]) -> Aes256Ctr128 {
        let mut key = [0u8; 32];
        key[..16].copy_from_slice(&x[..16]);
        key[16..].copy_from_slice(&y[16..]);

        let mut ctr = [0u8; 16];
        ctr[..4].copy_from_slice(&y[..4]);
        ctr[4..].copy_from_slice(&x[20..]);

        Aes256Ctr128::new(
            GenericArray::from_slice(&key),
            GenericArray::from_slice(&ctr),
        )
    }
}
