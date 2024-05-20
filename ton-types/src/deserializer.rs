use bitter::LittleEndianReader;
use bytes::Buf;
use thiserror::Error;
use bitter::BitReader;
use crate::cell::Cell;

#[derive(Debug, Error)]
pub enum DeserializerError {
    #[error("Unexpected constructor number: {0}")]
    UnexpectedConstructorNumber(u32),
    #[error("Input not empty after deserialization")]
    InputNotEmpty
}

pub trait Deserialize where Self: Sized {
    fn deserialize(de: &mut Deserializer) -> Result<Self, DeserializerError>;
}

pub trait DeserializeBare<const CONSTRUCTOR_NUMBER: u32> where Self: Sized {
    fn deserialize_bare(de: &mut Deserializer) -> Result<Self, DeserializerError>;
}

pub struct Deserializer<'de> {
    input: &'de [u8]
}

impl<'de> Deserializer<'de> {
    pub fn new(input: &'de [u8]) -> Self {
        Self { input }
    }

    pub fn bit_reader(&mut self) -> LittleEndianReader {
        LittleEndianReader::new(self.input)
    }

    pub fn parse_constructor_numer(&mut self) -> Result<u32, DeserializerError> {
        Ok(self.input.get_u32())
    }

    pub fn parse_u8(&mut self) -> Result<u8, DeserializerError> {
        Ok(self.input.get_u8())
    }

    pub fn parse_u32(&mut self) -> Result<u32, DeserializerError> {
        Ok(self.input.get_u32_le())
    }

    pub fn parse_sized_u32(&mut self, size: usize) -> Result<u32, DeserializerError> {
        let mut buffer = [0u8; 4];
        self.input.copy_to_slice(&mut buffer[4 - size .. ]);

        Ok(u32::from_be_bytes(buffer))
    }

    pub fn parse_sized_u64(&mut self, size: usize) -> Result<u64, DeserializerError> {
        let mut buffer = [0u8; 8];
        self.input.copy_to_slice(&mut buffer[8 - size ..]);

        Ok(u64::from_be_bytes(buffer))
    }

    pub fn parse_u8_vec(&mut self, size: usize) -> Result<Vec<u8>, DeserializerError> {
        let mut result = vec![0; size];
        self.input.copy_to_slice(&mut result);

        Ok(result)
    }

    pub fn parse_cell(&mut self) -> Result<Cell, DeserializerError> {
        let refs_descriptor = self.input.get_u8();
        let bits_descriptor = self.input.get_u8();

        let refs_count = refs_descriptor & 0b00000111;

        // bits_descriptor is the number of 4-bit groups in content,
        // so we need to divide it by 2 to get the number of bytes in content,
        // but we also need to add 1 if bits_descriptor is odd
        let len = ((bits_descriptor / 2) + (bits_descriptor % 2)) as usize;

        let mut content = vec![0; len];
        self.input.copy_to_slice(&mut content);

        // if bits_descriptor is odd, we need to clear the least significant bit of the last byte in content
        if bits_descriptor % 2 > 0 {
            content[len - 1] = content[len - 1] & (content[len - 1] - 1);
        }

        let mut refs = vec![0; refs_count as usize];
        self.input.copy_to_slice(&mut refs);

        Ok(Cell::new(content, refs))
    }
}

pub fn from_bytes<T>(bytes: &[u8]) -> Result<T, DeserializerError> where T: Deserialize {
    let mut deserializer = Deserializer::new(bytes);
    let t = T::deserialize(&mut deserializer)?;
    if deserializer.input.is_empty() {
        Ok(t)
    } else {
        Err(DeserializerError::InputNotEmpty)
    }
}
