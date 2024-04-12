use anyhow::bail;
use bytes::{Buf, Bytes};
use crate::types::Int256;

pub trait Deserialize where Self: Sized {
    fn deserialize(de: &mut Deserializer) -> anyhow::Result<Self>;
}

pub struct Deserializer {
    input: Bytes,
    peek_constructor_number: Option<u32>
}

impl Deserializer {
    // TODO[akostylev0]
    pub fn from_bytes(bytes: Vec<u8>) -> Self {
        Deserializer { input: bytes.into(), peek_constructor_number: None }
    }

    pub fn unpeek_constructor_number(&mut self, constructor_number: u32) {
        self.peek_constructor_number.replace(constructor_number);
    }

    pub fn parse_constructor_numer(&mut self) -> anyhow::Result<u32> {
        Ok(match self.peek_constructor_number.take() {
            Some(c) => c,
            None => self.input.get_u32()
        })
    }

    pub fn verify_constructor_number(&mut self, crc32: u32) -> anyhow::Result<()> {
        let constructor_number = match self.peek_constructor_number {
            Some(c) => { c }
            None => { self.input.get_u32() }
        };

        if constructor_number == crc32 {
            Ok(())
        } else {
            bail!("Unexpected constructor number, expected: {}, actual: {}", crc32, constructor_number)
        }
    }

    pub fn parse_bool(&mut self) -> anyhow::Result<bool> {
        unimplemented!()
    }

    pub fn parse_i31(&mut self) -> anyhow::Result<i32> {
        Ok(self.input.get_i32_le() & 0x7fffffff)
    }

    pub fn parse_i32(&mut self) -> anyhow::Result<i32> {
        Ok(self.input.get_i32_le())
    }

    pub fn parse_i64(&mut self) -> anyhow::Result<i64> {
        Ok(self.input.get_i64_le())
    }

    pub fn parse_i256(&mut self) -> anyhow::Result<Int256> {
        let mut needed = self.input.split_to(32);
        let mut result: [u8; 32] = [0; 32];
        needed.copy_to_slice(&mut result);

        Ok(result)
    }

    pub fn parse_bytes(&mut self) -> anyhow::Result<crate::types::Bytes> {
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
            needed.copy_to_slice(&mut (len[..3]));
            let len = u32::from_le_bytes(len);

            let mut needed = self.input.split_to(len as usize);
            let mut result = vec![0; len as usize];

            needed.copy_to_slice(&mut result);
            let padding = len % 4;
            if padding > 0 {
                self.input.advance(4 - padding as usize)
            }

            Ok(result)
        }
    }

    pub fn parse_string(&mut self) -> anyhow::Result<String> {
        let bytes = self.parse_bytes()?;

        Ok(String::from_utf8(bytes)?)
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
    use super::*;

    #[test]
    fn deserialize_bytes_length255() {
        let mut buf = vec![254, 255, 0, 0];
        buf.append(&mut vec![1; 255]);
        buf.append(&mut vec![0; 1]);
        let mut deserializer = Deserializer::from_bytes(buf);

        let value = deserializer.parse_bytes().unwrap();

        assert_eq!(value, vec![1; 255])
    }
}

