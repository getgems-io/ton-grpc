use std::fmt::{Debug, Formatter};
use rand::random;
use sha2::{Digest, Sha256, digest::Update};

#[derive(PartialEq, Eq)]
pub struct Packet {
    pub nonce: [u8; 32],
    pub checksum: [u8; 32],
    pub data: Vec<u8>,
}

impl Packet {
    pub fn empty() -> Self {
        Self::new(vec![])
    }

    pub fn new(data: Vec<u8>) -> Self {
        let nonce: [u8; 32] = random();

        let checksum: [u8; 32] = Sha256::default()
            .chain(nonce)
            .chain(&data)
            .finalize()
            .into();

        Self { nonce, data, checksum }
    }

    pub fn is_empty(&self) -> bool {
        self.data.is_empty()
    }

    pub fn len(&self) -> usize {
        self.data.len()
    }
}

impl Debug for Packet {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Packet")
            .field("checksum", &hex::encode(self.checksum))
            .field("data", &hex::encode(&self.data))
            .field("length", &self.data.len())
            .finish()
    }
}
