use adnl_tcp::deserializer::DeserializeBoxed;
use adnl_tcp::serializer::{SerializeBoxed};
use adnl_tcp::types::{Functional, Int, Long};
use crate::tl::{LiteServerGetAllShardsInfo, LiteServerGetBlock, LiteServerGetBlockHeader, LiteServerLookupBlock, TonNodeBlockId, TonNodeBlockIdExt};

pub trait Requestable: SerializeBoxed + Send {
    type Response: DeserializeBoxed + Send + 'static;
}

impl<T> Requestable for T
    where T : Functional + SerializeBoxed + Send,
        T::Result: DeserializeBoxed + Send + 'static {
    type Response = T::Result;
}

impl TonNodeBlockId {
    pub fn new(workchain: Int, shard: Long, seqno: Int) -> Self {
        Self { workchain, shard, seqno }
    }
}

impl LiteServerLookupBlock {
    pub fn seqno(block_id: TonNodeBlockId) -> Self {
        Self {
            mode: 1,
            id: block_id,
            lt: None,
            utime: None,
        }
    }
}

impl LiteServerGetBlockHeader {
    pub fn new(id: TonNodeBlockIdExt) -> Self {
        Self { id, mode: 0 }
    }
}

impl LiteServerGetBlock {
    pub fn new(id: TonNodeBlockIdExt) -> Self {
        Self { id }
    }
}

impl LiteServerGetAllShardsInfo {
    pub fn new(block_id: TonNodeBlockIdExt) -> Self {
        Self { id: block_id }
    }
}
