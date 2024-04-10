use std::error::Error;
use std::fmt::{Debug, Display};
use bytes::BufMut;
use crate::tl::{Bytes, Int256};

#[derive(Debug)]
pub struct Serializer {
    output: Vec<u8>,
}

impl Serializer {
    pub fn write_constructor_number(&mut self, crc32: u32) {
        self.output.put_u32(crc32)
    }

    pub fn write_bytes(&mut self, val: &Bytes) {
        if val.len() <= 253 {
            let padding = (val.len() + 1) % 4;
            if padding > 0 {
                self.output.reserve(val.len() + 1 + 4 - padding);
                self.output.put_u8(val.len() as u8);
                self.output.put_slice(val);
                self.output.put_bytes(0, 4 - padding);
            } else {
                self.output.reserve(val.len() + 1);
                self.output.put_u8(val.len() as u8);
                self.output.put_slice(val);
            }
        } else {
            let mut padding = (val.len() + 4) % 4;
            if padding > 0 {
                self.output.reserve(val.len() + 4 + 4 - padding);
                self.output.put_u8(254);
                self.output.put_slice(&(val.len() as u32).to_be_bytes()[1..]);
                self.output.put_slice(val);
                self.output.put_bytes(0, 4 - padding);
            } else {
                self.output.reserve(val.len() + 4);
                self.output.put_u8(254);
                self.output.put_slice(&(val.len() as u32).to_be_bytes()[1..]);
                self.output.put_u8(val.len() as u8);
                self.output.put_slice(val);
            }
        }
    }

    pub fn write_i256(&mut self, val: &Int256) {
        self.output.reserve(32);
        self.output.put_slice(val)
    }
}

pub trait Serialize {
    fn serialize(&self, serializer: &mut Serializer) -> anyhow::Result<()>;
}

pub fn to_bytes<T>(value: &T) -> anyhow::Result<Vec<u8>>
    where
        T: Serialize,
{
    let mut serializer = Serializer { output: Vec::new() };
    value.serialize(&mut serializer)?;

    Ok(serializer.output)
}

#[cfg(test)]
mod tests {
    use std::process::exit;
    use crate::tl::{AdnlMessageQuery, LiteServerQuery, Bytes, Int256};
    use super::*;

    #[test]
    #[tracing_test::traced_test]
    fn serialize_bytes_length255() {
        let mut serializer = Serializer { output: Vec::new() };
        let value = vec![1; 255];
        let mut expected = vec![254, 0, 0, 255];
        expected.append(&mut vec![1; 255]);
        expected.append(&mut vec![0; 1]);

        serializer.write_bytes(&value);

        assert_eq!(serializer.output, expected)
    }

    #[test]
    #[tracing_test::traced_test]
    fn adnl_query_serialize_test() {
        let query = AdnlMessageQuery {
            query_id: hex::decode("77c1545b96fa136b8e01cc08338bec47e8a43215492dda6d4d7e286382bb00c4").unwrap().try_into().unwrap(),
            query: hex::decode("df068c79042ee6b589000000").unwrap()
        };

        let bytes = to_bytes(&query).unwrap();

        assert_eq!(bytes, hex::decode("7af98bb477c1545b96fa136b8e01cc08338bec47e8a43215492dda6d4d7e286382bb00c40cdf068c79042ee6b589000000000000").unwrap())
    }

    #[test]
    #[tracing_test::traced_test]
    fn liteserver_query_serialize_test() {
        let query = LiteServerQuery {
            data: hex::decode("2ee6b589").unwrap(),
        };

        let bytes = to_bytes(&query).unwrap();

        assert_eq!(bytes, hex::decode("df068c79042ee6b589000000").unwrap())
    }
}