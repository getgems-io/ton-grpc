use crate::deserializer::Deserialize;
use crate::serializer::{Serialize, Serializer};

pub trait Functional {
    type Result;
}

type Double = f64;
type Int31 = i32; // "#" / nat type
type Int32 = i32;
type Int = i32;
type Int53 = i64;
type Int64 = i64;
type Long = i64;
type Int128 = i128;
pub type Int256 = [u8; 32];
type BoxedBool = bool;
pub type Bytes = Vec<u8>;
type Object = Bytes;
type SecureString = String;
type SecureBytes = Vec<u8>;
type Vector<T> = Vec<T>;

include!(concat!(env!("OUT_DIR"), "/generated.rs"));


impl Serialize for AdnlMessageQuery {
    fn serialize(&self, se: &mut Serializer) -> anyhow::Result<()> {
        se.write_constructor_number(Self::CONSTRUCTOR_NUMBER_BE);
        se.write_i256(&self.query_id);
        se.write_bytes(&self.query);

        Ok(())
    }
}

impl Deserialize for AdnlMessageQuery {
    fn deserialize(de: &mut crate::deserializer::Deserializer) -> anyhow::Result<Self> {
        de.verify_constructor_number(Self::CONSTRUCTOR_NUMBER_BE)?;

        let query_id = de.parse_i256()?;
        let query = de.parse_bytes()?;

        Ok(Self {
            query_id,
            query
        })
    }
}

impl Serialize for LiteServerQuery {
    fn serialize(&self, se: &mut Serializer) -> anyhow::Result<()> {
        se.write_constructor_number(Self::CONSTRUCTOR_NUMBER_BE);
        se.write_bytes(&self.data);

        Ok(())
    }
}

impl Serialize for LiteServerGetMasterchainInfo {
    fn serialize(&self, se: &mut Serializer) -> anyhow::Result<()> {
        se.write_constructor_number(Self::CONSTRUCTOR_NUMBER_BE);

        Ok(())
    }
}
