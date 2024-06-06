use adnl_tcp::deserializer::DeserializeBoxed;
use adnl_tcp::serializer::{SerializeBoxed, Serializer};
use adnl_tcp::types::{Functional, Int, Long};
use crate::tl::{LiteServerGetBlock, LiteServerLookupBlock, LiteServerWaitMasterchainSeqno, TonNodeBlockId, TonNodeBlockIdExt};

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
        Self::with_timeout(request, seqno, 3000)
    }

    pub fn with_timeout(request: R, seqno: i32, timeout_ms: i32) -> Self {
        Self { prefix: LiteServerWaitMasterchainSeqno { seqno, timeout_ms }, request }
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

impl LiteServerGetBlock {
    pub fn new(id: TonNodeBlockIdExt) -> Self {
        Self { id }
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
