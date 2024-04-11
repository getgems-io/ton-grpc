use std::fmt::Debug;
use bytes::BufMut;
use crate::types::{Bytes, Int256, String};

pub trait Serialize {
    fn serialize(&self, se: &mut Serializer) -> anyhow::Result<()>;
}

#[derive(Debug)]
pub struct Serializer {
    output: Vec<u8>,
}

impl Serializer {
    pub fn write_constructor_number(&mut self, crc32: u32) {
        self.output.put_u32(crc32)
    }

    pub fn write_bool(&mut self, val: bool) {
        unimplemented!()
    }

    pub fn write_i32(&mut self, val: i32) {
        self.output.put_i32_le(val)
    }

    pub fn write_i31(&mut self, val: i32) {
        unimplemented!()
    }

    pub fn write_i64(&mut self, val: i64) {
        self.output.put_i64_le(val)
    }

    pub fn write_i256(&mut self, val: &Int256) {
        self.output.reserve(32);
        self.output.put_slice(val)
    }

    pub fn write_string(&mut self, val: &String) {
        unimplemented!()
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
            let padding = (val.len() + 4) % 4;
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
    use super::*;

    #[test]
    fn serialize_bytes_length255() {
        let mut serializer = Serializer { output: Vec::new() };
        let value = vec![1; 255];
        let mut expected = vec![254, 0, 0, 255];
        expected.append(&mut vec![1; 255]);
        expected.append(&mut vec![0; 1]);

        serializer.write_bytes(&value);

        assert_eq!(serializer.output, expected)
    }
}
