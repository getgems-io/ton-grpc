pub trait Deserialize {
    fn deserialize(de: &mut Deserializer) -> anyhow::Result<Self>;
}

struct Deserializer {
    input: Vec<u8>
}

impl Deserializer {
    pub fn from_bytes(bytes: Vec<u8>) -> Self {
        return Deserializer { input: bytes }
    }
}

