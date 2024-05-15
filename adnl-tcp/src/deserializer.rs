use anyhow::bail;
use bytes::Buf;
use thiserror::Error;
use crate::types::Int256;

#[derive(Error, Debug)]
pub enum DeserializerBoxedError {
    #[error("Unexpected constructor number: {0}")]
    UnexpectedConstructorNumber(u32),
    #[error(transparent)]
    DeserializeError(#[from] anyhow::Error)
}

pub trait Deserialize where Self: Sized {
    fn deserialize(de: &mut Deserializer) -> Result<Self, DeserializerBoxedError>;
}

pub trait DeserializeBoxed: Deserialize {
    fn deserialize_boxed(constructor_number: u32, de: &mut Deserializer) -> Result<Self, DeserializerBoxedError>;
}

pub struct Deserializer<'de> {
    input: &'de [u8]
}

impl<'de> Deserializer<'de> {
    pub fn from_bytes(input: &'de [u8]) -> Self {
        Deserializer { input }
    }

    pub fn parse_constructor_numer(&mut self) -> anyhow::Result<u32> {
        Ok(self.input.get_u32())
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
        let mut result: [u8; 32] = [0; 32];
        self.input.copy_to_slice(&mut result);

        Ok(result)
    }

    pub fn parse_bytes(&mut self) -> anyhow::Result<crate::types::Bytes> {
        let len = self.input.get_u8();
        if len <= 253 {
            let mut result = vec![0; len as usize];
            self.input.copy_to_slice(&mut result);

            let padding = (len + 1) % 4;
            if padding > 0 {
                self.input.advance(4 - padding as usize)
            }

            Ok(result)
        } else {
            let mut len: [u8; 4] = [0; 4];
            self.input.copy_to_slice(&mut (len[..3]));
            let len = u32::from_le_bytes(len);

            let mut result = vec![0; len as usize];
            self.input.copy_to_slice(&mut result);

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

pub fn from_bytes_boxed<T>(bytes: &[u8]) -> anyhow::Result<T>
    where T: DeserializeBoxed,
{
    let mut deserializer = Deserializer::from_bytes(bytes);
    let constructor_number = deserializer.parse_constructor_numer()?;
    let t = T::deserialize_boxed(constructor_number, &mut deserializer)?;
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
        let mut deserializer = Deserializer::from_bytes(&buf);

        let value = deserializer.parse_bytes().unwrap();

        assert_eq!(value, vec![1; 255])
    }
}
