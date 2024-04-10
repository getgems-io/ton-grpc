use anyhow::bail;
use bytes::{Buf, Bytes};
use crate::tl::Int256;

pub trait Deserialize where Self: Sized {
    fn deserialize(de: &mut Deserializer) -> anyhow::Result<Self>;
}

pub struct Deserializer {
    input: Bytes
}

impl Deserializer {
    // TODO[akostylev0]
    pub fn from_bytes(bytes: Vec<u8>) -> Self {
        return Deserializer { input: bytes.into() }
    }

    pub fn verify_constructor_number(&mut self, crc32: u32) -> anyhow::Result<()> {
        if self.input.get_u32() == crc32 {
            Ok(())
        } else {
            bail!("Unexpected constructor number")
        }
    }

    pub fn parse_i256(&mut self) -> anyhow::Result<Int256> {
        if self.input.remaining() >= 32 {
            let mut needed = self.input.split_to(32);
            let mut result: [u8; 32] = [0; 32];
            needed.copy_to_slice(&mut result);

            return Ok(result)
        }

        bail!("not enough bytes")
    }

    pub fn parse_bytes(&mut self) -> anyhow::Result<crate::tl::Bytes> {
        let len = self.input.get_u8();
        if len <= 253 {
            let mut needed = self.input.split_to(len as usize);
            let mut result = vec![0; len as usize];
            needed.copy_to_slice(&mut result);
            let padding = (len + 1) % 4;
            if padding > 0 {
                self.input.advance(4 - padding as usize)
            }

            Ok(result)
        } else {
            let mut len: [u8; 4] = [0; 4];
            let mut needed = self.input.split_to(3);
            needed.copy_to_slice(&mut (len[1..]));
            let len = u32::from_be_bytes(len);

            let mut needed = self.input.split_to(len as usize);
            let mut result = vec![0; len as usize];

            needed.copy_to_slice(&mut result);
            let padding = (len + 1) % 4;
            if padding > 0 {
                self.input.advance(4 - padding as usize)
            }

            Ok(result)
        }
    }
}

pub fn from_bytes<T>(bytes: Vec<u8>) -> anyhow::Result<T>
    where
        T: Deserialize,
{
    let mut deserializer = Deserializer::from_bytes(bytes);
    let t = T::deserialize(&mut deserializer)?;
    if deserializer.input.is_empty() {
        Ok(t)
    } else {
        bail!("input is not empty")
    }
}


#[cfg(test)]
mod tests {
    use crate::serializer::Serializer;
    use crate::tl::{AdnlMessageQuery, LiteServerQuery, Bytes, Int256, LiteServerGetMasterchainInfo};
    use super::*;

    #[test]
    #[tracing_test::traced_test]
    fn deserialize_adnl_query_test() {
        let bytes = hex::decode("7af98bb477c1545b96fa136b8e01cc08338bec47e8a43215492dda6d4d7e286382bb00c40cdf068c79042ee6b589000000000000").unwrap();

        let query = from_bytes::<AdnlMessageQuery>(bytes).unwrap();

        assert_eq!(query, AdnlMessageQuery {
            query_id: hex::decode("77c1545b96fa136b8e01cc08338bec47e8a43215492dda6d4d7e286382bb00c4").unwrap().try_into().unwrap(),
            query: hex::decode("df068c79042ee6b589000000").unwrap()
        })
    }

    #[test]
    #[tracing_test::traced_test]
    fn deserialize_bytes_length255() {
        let mut buf = vec![254, 0, 0, 255];
        buf.append(&mut vec![1; 255]);
        buf.append(&mut vec![0; 1]);
        let mut deserializer = Deserializer { input: buf.into() };

        let value = deserializer.parse_bytes().unwrap();

        assert_eq!(value, vec![1; 255])
    }
}

