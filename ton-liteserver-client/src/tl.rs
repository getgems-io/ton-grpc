use crate::deserializer::{Deserialize, Deserializer};
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

// TODO[akostylev0] move writing/reading constructor number to boxed types

impl Serialize for AdnlMessageQuery {
    fn serialize(&self, se: &mut Serializer) -> anyhow::Result<()> {
        se.write_constructor_number(Self::CONSTRUCTOR_NUMBER_BE);
        se.write_i256(&self.query_id);
        se.write_bytes(&self.query);

        Ok(())
    }
}

impl Deserialize for AdnlMessageQuery {
    fn deserialize(de: &mut Deserializer) -> anyhow::Result<Self> {
        de.verify_constructor_number(Self::CONSTRUCTOR_NUMBER_BE)?;

        let query_id = de.parse_i256()?;
        let query = de.parse_bytes()?;

        Ok(Self {
            query_id,
            query
        })
    }
}

impl Serialize for AdnlMessageAnswer {
    fn serialize(&self, se: &mut Serializer) -> anyhow::Result<()> {
        se.write_constructor_number(Self::CONSTRUCTOR_NUMBER_BE);
        se.write_bytes(&self.answer);

        Ok(())
    }
}

impl Deserialize for AdnlMessageAnswer {
    fn deserialize(de: &mut Deserializer) -> anyhow::Result<Self> {
        de.verify_constructor_number(Self::CONSTRUCTOR_NUMBER_BE)?;
        let query_id = de.parse_i256()?;
        let answer = de.parse_bytes()?;

        Ok(Self {
            query_id,
            answer
        })
    }
}

impl Serialize for TonNodeBlockIdExt {
    fn serialize(&self, se: &mut Serializer) -> anyhow::Result<()> {
        se.write_i32(self.workchain);
        se.write_i64(self.shard);
        se.write_i32(self.seqno);
        se.write_i256(&self.root_hash);
        se.write_i256(&self.file_hash);

        Ok(())
    }
}

impl Deserialize for TonNodeBlockIdExt {
    fn deserialize(de: &mut Deserializer) -> anyhow::Result<Self> {
        let workchain = de.parse_i32()?;
        let shard = de.parse_i64()?;
        let seqno = de.parse_i32()?;
        let root_hash = de.parse_i256()?;
        let file_hash = de.parse_i256()?;

        Ok(Self {
            workchain,
            shard,
            seqno,
            root_hash,
            file_hash
        })
    }
}

impl Serialize for TonNodeZeroStateIdExt {
    fn serialize(&self, se: &mut Serializer) -> anyhow::Result<()> {
        se.write_i32(self.workchain);
        se.write_i256(&self.root_hash);
        se.write_i256(&self.file_hash);

        Ok(())
    }
}

impl Deserialize for TonNodeZeroStateIdExt {
    fn deserialize(de: &mut Deserializer) -> anyhow::Result<Self> {
        let workchain = de.parse_i32()?;
        let root_hash = de.parse_i256()?;
        let file_hash = de.parse_i256()?;

        Ok(Self {
            workchain,
            root_hash,
            file_hash
        })
    }
}

impl Serialize for LiteServerMasterchainInfo {
    fn serialize(&self, se: &mut Serializer) -> anyhow::Result<()> {
        se.write_constructor_number(Self::CONSTRUCTOR_NUMBER_BE);
        self.last.serialize(se)?;
        se.write_i256(&self.state_root_hash);
        self.init.serialize(se)?;

        Ok(())
    }
}

impl Deserialize for LiteServerMasterchainInfo {
    fn deserialize(de: &mut Deserializer) -> anyhow::Result<Self> {
        de.verify_constructor_number(Self::CONSTRUCTOR_NUMBER_BE)?;
        let last = TonNodeBlockIdExt::deserialize(de)?;
        let state_root_hash = de.parse_i256()?;
        let init = TonNodeZeroStateIdExt::deserialize(de)?;

        Ok(Self {
            last,
            state_root_hash,
            init
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

impl Deserialize for LiteServerQuery {
    fn deserialize(de: &mut Deserializer) -> anyhow::Result<Self> {
        de.verify_constructor_number(Self::CONSTRUCTOR_NUMBER_BE)?;
        let data = de.parse_bytes()?;

        Ok(Self {
            data
        })
    }
}

impl Serialize for LiteServerGetMasterchainInfo {
    fn serialize(&self, se: &mut Serializer) -> anyhow::Result<()> {
        se.write_constructor_number(Self::CONSTRUCTOR_NUMBER_BE);

        Ok(())
    }
}

impl Deserialize for LiteServerGetMasterchainInfo {
    fn deserialize(de: &mut Deserializer) -> anyhow::Result<Self> {
        de.verify_constructor_number(Self::CONSTRUCTOR_NUMBER_BE)?;

        Ok(Self {})
    }
}


#[cfg(test)]
mod tests {
    use crate::deserializer::from_bytes;
    use crate::serializer::to_bytes;
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
    fn serialize_adnl_query_test() {
        let query = AdnlMessageQuery {
            query_id: hex::decode("77c1545b96fa136b8e01cc08338bec47e8a43215492dda6d4d7e286382bb00c4").unwrap().try_into().unwrap(),
            query: hex::decode("df068c79042ee6b589000000").unwrap()
        };

        let bytes = to_bytes(&query).unwrap();

        assert_eq!(bytes, hex::decode("7af98bb477c1545b96fa136b8e01cc08338bec47e8a43215492dda6d4d7e286382bb00c40cdf068c79042ee6b589000000000000").unwrap())
    }

    #[test]
    fn serialize_liteserver_query_test() {
        let query = LiteServerQuery {
            data: hex::decode("2ee6b589").unwrap(),
        };

        let bytes = to_bytes(&query).unwrap();

        assert_eq!(bytes, hex::decode("df068c79042ee6b589000000").unwrap())
    }

    #[test]
    fn serialize_get_masterchain_info_test() {
        let s = LiteServerGetMasterchainInfo {};

        let bytes = to_bytes(&s).unwrap();

        assert_eq!(bytes, hex::decode("2ee6b589").unwrap())
    }

    #[test]
    fn deserialize_masterchain_info_test() {

    }
}