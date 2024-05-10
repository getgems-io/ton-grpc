use crate::boxed::Boxed;
use crate::deserializer::{Deserialize, Deserializer};
use crate::serializer::{Serialize, Serializer};

pub trait Functional {
    type Result;
}

pub trait BareType where Self: Sized {
    const CONSTRUCTOR_NUMBER_BE: u32;

    fn into_boxed(self) -> Boxed<Self> {
        Boxed::new(self)
    }
}

pub trait BoxedType where Self: Sized {
    fn constructor_number(&self) -> u32;
}

// TODO[akostylev0] review
pub type Double = f64;
pub type Int31 = i32; // "#" / nat type
pub type Int32 = i32;
pub type Int = i32;
pub type Int53 = i64;
pub type Int64 = i64;
pub type Long = i64;
pub type Int128 = i128;
pub type Int256 = [u8; 32];
pub type Bytes = Vec<u8>;
pub type Object = Bytes;
pub type SecureString = String;
pub type SecureBytes = Vec<u8>;
pub type Vector<T> = Vec<T>;

impl Serialize for Vector<Int256> {
    fn serialize(&self, se: &mut Serializer) {
        se.reserve(4 + 32 * self.len());
        se.write_i31(self.len() as i32);
        for val in self {
            se.write_i256(val)
        }
    }
}

impl Deserialize for Vector<Int256> {
    fn deserialize(de: &mut Deserializer) -> anyhow::Result<Self> {
        let len = de.parse_i31()?;
        let mut buf = Vec::with_capacity(len as usize);
        for _ in 0 .. len {
            let val = de.parse_i256()?;
            buf.push(val)
        }

        Ok(buf)
    }
}

impl Serialize for Vector<Int32> {
    fn serialize(&self, se: &mut Serializer) {
        se.reserve(4 + 4 * self.len());
        se.write_i31(self.len() as i32);
        for val in self {
            se.write_i32(*val)
        }
    }
}

impl Deserialize for Vector<Int32> {
    fn deserialize(de: &mut Deserializer) -> anyhow::Result<Self> {
        let len = de.parse_i31()?;
        let mut buf = Vec::with_capacity(len as usize);
        for _ in 0 .. len {
            let val = de.parse_i32()?;
            buf.push(val)
        }

        Ok(buf)
    }
}

impl Serialize for Vector<Int64> {
    fn serialize(&self, se: &mut Serializer) {
        se.reserve(4 + 8 * self.len());
        se.write_i31(self.len() as i32);
        for val in self {
            se.write_i64(*val)
        }
    }
}

impl Deserialize for Vector<Int64> {
    fn deserialize(de: &mut Deserializer) -> anyhow::Result<Self> {
        let len = de.parse_i31()?;
        let mut buf = Vec::with_capacity(len as usize);
        for _ in 0 .. len {
            let val = de.parse_i64()?;
            buf.push(val)
        }

        Ok(buf)
    }
}

impl<T> Serialize for Vector<T> where T : Serialize {
    fn serialize(&self, se: &mut Serializer) {
        se.write_i31(self.len() as i32);
        for val in self {
            val.serialize(se)
        }
    }
}

impl<T> Deserialize for Vector<T> where T : Deserialize {
    fn deserialize(de: &mut Deserializer) -> anyhow::Result<Self> {
        let len = de.parse_i31()?;
        let mut buf = Vec::with_capacity(len as usize);
        for _ in 0 .. len {
            let val = T::deserialize(de)?;
            buf.push(val)
        }

        Ok(buf)
    }
}

impl<T, E> Deserialize for Result<T, E> where T: Deserialize, E: BareType + Deserialize {
    fn deserialize(de: &mut Deserializer) -> anyhow::Result<Self> {
        let constructor_number = de.parse_constructor_numer()?;

        if constructor_number == E::CONSTRUCTOR_NUMBER_BE {
            return Ok(Err(E::deserialize(de)?))
        }

        // Put back constructor number for T::deserialize
        de.unpeek_constructor_number(constructor_number);

        Ok(Ok(T::deserialize(de)?))
    }
}

impl<T, E> Serialize for Result<T, E> where T: Serialize, E: BareType + Serialize {
    fn serialize(&self, se: &mut Serializer) {
        match self {
            Ok(val) => {
                val.serialize(se);
            }
            Err(val) => {
                se.write_constructor_number(E::CONSTRUCTOR_NUMBER_BE);
                val.serialize(se);
            }
        }
    }
}
