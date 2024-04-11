#![allow(dead_code)]

use adnl_tcp::deserializer::{Deserialize, Deserializer};
use adnl_tcp::serializer::{Serialize, Serializer};
use adnl_tcp::boxed::Boxed;
pub use adnl_tcp::types::*;

include!(concat!(env!("OUT_DIR"), "/generated.rs"));

// TODO[akostylev0] move writing/reading constructor number to boxed types

impl Serialize for AdnlMessageQuery {
    fn serialize(&self, se: &mut Serializer) -> anyhow::Result<()> {
        se.write_i256(&self.query_id);
        se.write_bytes(&self.query);

        Ok(())
    }
}

impl Deserialize for AdnlMessageQuery {
    fn deserialize(de: &mut Deserializer) -> anyhow::Result<Self> {
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
        se.write_bytes(&self.answer);

        Ok(())
    }
}

impl Deserialize for AdnlMessageAnswer {
    fn deserialize(de: &mut Deserializer) -> anyhow::Result<Self> {
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
        self.last.serialize(se)?;
        se.write_i256(&self.state_root_hash);
        self.init.serialize(se)?;

        Ok(())
    }
}

impl Deserialize for LiteServerMasterchainInfo {
    fn deserialize(de: &mut Deserializer) -> anyhow::Result<Self> {
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
        se.write_bytes(&self.data);

        Ok(())
    }
}

impl Deserialize for LiteServerQuery {
    fn deserialize(de: &mut Deserializer) -> anyhow::Result<Self> {
        let data = de.parse_bytes()?;

        Ok(Self {
            data
        })
    }
}

impl Serialize for LiteServerGetMasterchainInfo {
    fn serialize(&self, _: &mut Serializer) -> anyhow::Result<()> {
        Ok(())
    }
}

impl Deserialize for LiteServerGetMasterchainInfo {
    fn deserialize(_: &mut Deserializer) -> anyhow::Result<Self> {
        Ok(Self {})
    }
}


#[cfg(test)]
mod tests {
    use adnl_tcp::boxed::Boxed;
    use adnl_tcp::deserializer::from_bytes;
    use adnl_tcp::serializer::to_bytes;
    use super::*;

    #[test]
    fn serialize_adnl_query_test() {
        let query = AdnlMessageQuery {
            query_id: hex::decode("77c1545b96fa136b8e01cc08338bec47e8a43215492dda6d4d7e286382bb00c4").unwrap().try_into().unwrap(),
            query: hex::decode("df068c79042ee6b589000000").unwrap()
        };

        let bytes = to_bytes(&query.into_boxed()).unwrap();

        assert_eq!(bytes, hex::decode("7af98bb477c1545b96fa136b8e01cc08338bec47e8a43215492dda6d4d7e286382bb00c40cdf068c79042ee6b589000000000000").unwrap())
    }

    #[test]
    fn serialize_liteserver_query_test() {
        let query = LiteServerQuery {
            data: hex::decode("2ee6b589").unwrap(),
        };

        let bytes = to_bytes(&query.into_boxed()).unwrap();

        assert_eq!(bytes, hex::decode("df068c79042ee6b589000000").unwrap())
    }

    #[test]
    fn serialize_get_masterchain_info_test() {
        let s = LiteServerGetMasterchainInfo {};

        let bytes = to_bytes(&s.into_boxed()).unwrap();

        assert_eq!(bytes, hex::decode("2ee6b589").unwrap())
    }

    #[test]
    #[tracing_test::traced_test]
    fn deserialize_adnl_query_test() {
        let bytes = hex::decode("7af98bb477c1545b96fa136b8e01cc08338bec47e8a43215492dda6d4d7e286382bb00c40cdf068c79042ee6b589000000000000").unwrap();

        let query = from_bytes::<Boxed<AdnlMessageQuery>>(bytes).unwrap();

        assert_eq!(query, AdnlMessageQuery {
            query_id: hex::decode("77c1545b96fa136b8e01cc08338bec47e8a43215492dda6d4d7e286382bb00c4").unwrap().try_into().unwrap(),
            query: hex::decode("df068c79042ee6b589000000").unwrap()
        }.into_boxed())
    }

    #[test]
    fn deserialize_masterchain_info_test() {
        let bytes = hex::decode("81288385ffffffff000000000000008027405801e585a47bd5978f6a4fb2b56aa2082ec9deac33aaae19e78241b97522e1fb43d4876851b60521311853f59c002d46b0bd80054af4bce340787a00bd04e01235178b4d3b38b06bb484015faf9821c3ba1c609a25b74f30e1e585b8c8e820ef0976ffffffff17a3a92992aabea785a7a090985a265cd31f323d849da51239737e321fb055695e994fcf4d425c0a6ce6a792594b7173205f740a39cd56f537defd28b48a0f6e").unwrap();

        let masterchain_info = from_bytes::<Boxed<LiteServerMasterchainInfo>>(bytes).unwrap();

        eprintln!("{}", base64::encode(hex::decode("e585a47bd5978f6a4fb2b56aa2082ec9deac33aaae19e78241b97522e1fb43d4").unwrap()));
        eprintln!("{}", base64::encode(hex::decode("876851b60521311853f59c002d46b0bd80054af4bce340787a00bd04e0123517").unwrap()));

        assert_eq!(masterchain_info, LiteServerMasterchainInfo {
            last: TonNodeBlockIdExt {
                workchain: 0xffffffff_u32.to_be() as i32,
                shard: 0x00000000000080_u64.to_be() as i64,
                seqno: 0x27405801_u32.to_be() as i32,
                root_hash: hex::decode("e585a47bd5978f6a4fb2b56aa2082ec9deac33aaae19e78241b97522e1fb43d4").unwrap().try_into().unwrap(),
                file_hash: hex::decode("876851b60521311853f59c002d46b0bd80054af4bce340787a00bd04e0123517").unwrap().try_into().unwrap(),
            },
            state_root_hash: hex::decode("8b4d3b38b06bb484015faf9821c3ba1c609a25b74f30e1e585b8c8e820ef0976").unwrap().try_into().unwrap(),
            init: TonNodeZeroStateIdExt {
                workchain: 0xffffffff_u32.to_be() as i32,
                root_hash: hex::decode("17a3a92992aabea785a7a090985a265cd31f323d849da51239737e321fb05569").unwrap().try_into().unwrap(),
                file_hash: hex::decode("5e994fcf4d425c0a6ce6a792594b7173205f740a39cd56f537defd28b48a0f6e").unwrap().try_into().unwrap(),
            },
        }.into_boxed())
    }
}