use std::ops::Mul;
use rand::Rng;
use aes::cipher::generic_array::GenericArray;
use aes::cipher::KeyIvInit;
use aes::cipher::StreamCipher;
use anyhow::{anyhow, bail};
use ed25519_dalek::hazmat::ExpandedSecretKey;
use ed25519_dalek::VerifyingKey;
use sha2::{Digest, Sha256};

pub type Aes256Ctr128 = ctr::Ctr128BE<aes::Aes256>;

pub struct AesCtr {
    basis: [u8; 160]
}

impl AesCtr {
    pub fn generate() -> Self {
        let mut basis = [0u8; 160];
        rand::thread_rng().fill(basis.as_mut_slice());

        Self { basis }
    }

    pub fn from_encrypted(basis: &[u8; 160], checksum: &[u8; 32], expanded_secret_key: &ExpandedSecretKey, verifying_key: &VerifyingKey) -> anyhow::Result<Self> {
        let x = verifying_key.to_montgomery().mul(expanded_secret_key.scalar).to_bytes();

        let mut cipher = Self::cipher(&x, checksum);
        let mut basis_decrypted = [0u8; 160];
        cipher
            .apply_keystream_b2b(basis, &mut basis_decrypted)
            .map_err(|e| anyhow!(e))?;

        if Sha256::digest(basis_decrypted).as_slice() != checksum {
            bail!("wrong handshake checksum");
        }

        Ok(Self { basis: basis_decrypted })
    }

    pub fn into_bytes(self) -> [u8; 160] {
        self.basis
    }

    pub fn encrypt(&self, expanded_secret_key: &ExpandedSecretKey, verifying_key: &VerifyingKey) -> ([u8; 160], [u8; 32]) {
        let checksum: [u8; 32] = Sha256::digest(self.basis).into();
        let x = verifying_key.to_montgomery().mul(expanded_secret_key.scalar).to_bytes();

        let mut cipher = Self::cipher(&x, &checksum);
        let mut basis_encrypted = [0u8; 160];
        cipher
            .apply_keystream_b2b(&self.basis, &mut basis_encrypted)
            .unwrap();

        (basis_encrypted, checksum)
    }

    fn cipher(x: &[u8; 32], y: &[u8; 32]) -> Aes256Ctr128 {
        let key = [
            x[ 0], x[ 1], x[ 2], x[ 3], x[ 4], x[ 5], x[ 6], x[ 7],
            x[ 8], x[ 9], x[10], x[11], x[12], x[13], x[14], x[15],
            y[16], y[17], y[18], y[19], y[20], y[21], y[22], y[23],
            y[24], y[25], y[26], y[27], y[28], y[29], y[30], y[31]
        ];
        let ctr = [
            y[ 0], y[ 1], y[ 2], y[ 3], x[20], x[21], x[22], x[23],
            x[24], x[25], x[26], x[27], x[28], x[29], x[30], x[31]
        ];

        Aes256Ctr128::new(GenericArray::from_slice(&key), GenericArray::from_slice(&ctr))
    }
}
