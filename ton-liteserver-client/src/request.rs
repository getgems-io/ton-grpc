use std::time::Duration;
use adnl_tcp::deserializer::DeserializeBoxed;
use adnl_tcp::serializer::{SerializeBoxed, Serializer};
use adnl_tcp::types::{Functional, Int, Long};
use crate::tl::{LiteServerGetAllShardsInfo, LiteServerGetBlock, LiteServerGetBlockHeader, LiteServerLookupBlock, LiteServerWaitMasterchainSeqno, TonNodeBlockId, TonNodeBlockIdExt};

pub trait Requestable: SerializeBoxed + Send {
    type Response: DeserializeBoxed + Send + 'static;
}

impl<T> Requestable for T
    where T : Functional + SerializeBoxed + Send,
        T::Result: DeserializeBoxed + Send + 'static {
    type Response = T::Result;
}

pub struct WaitSeqno<R> {
    prefix: LiteServerWaitMasterchainSeqno,
    request: R,
}

impl<R> WaitSeqno<R> where R: Requestable {
    pub fn new(request: R, seqno: i32) -> Self {
        Self::with_timeout(request, seqno, Duration::from_secs(3))
    }

    pub fn with_timeout(request: R, seqno: i32, timeout: Duration) -> Self {
        Self { prefix: LiteServerWaitMasterchainSeqno { seqno, timeout_ms: timeout.as_millis() as i32 }, request }
    }
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

impl<R> SerializeBoxed for WaitSeqno<R> where R: Requestable {
    fn serialize_boxed(&self, se: &mut Serializer) {
        self.prefix.serialize_boxed(se);
        self.request.serialize_boxed(se);
    }
}

impl<R> Requestable for WaitSeqno<R> where R: Requestable {
    type Response = R::Response;
}
