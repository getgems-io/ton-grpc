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

impl<T, E> Deserialize for Result<T, E> where T:BoxedType + Deserialize, E: BareType + Deserialize {
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

impl<T, E> Serialize for Result<T, E> where T: BoxedType + Serialize, E: BareType + Serialize {
    fn serialize(&self, se: &mut Serializer) -> anyhow::Result<()> {
        match self {
            Ok(val) => {
                val.serialize(se)?;
                Ok(())
            }
            Err(val) => {
                se.write_constructor_number(E::CONSTRUCTOR_NUMBER_BE);
                val.serialize(se)?;
                Ok(())
            }
        }
    }
}