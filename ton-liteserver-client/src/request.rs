use adnl_tcp::deserializer::DeserializeBoxed;
use adnl_tcp::serializer::{SerializeBoxed, Serializer};
use adnl_tcp::types::Functional;
use crate::tl::{LiteServerWaitMasterchainSeqno, TonNodeBlockIdExt};

pub trait Requestable: SerializeBoxed + Send {
    type Response: DeserializeBoxed + Send + 'static;
}

pub trait TargetBlockId: Requestable {
    fn target_block_id(&self) -> &TonNodeBlockIdExt;
}

impl<T> Requestable for T
    where T : Functional + SerializeBoxed + Send,
        T::Result: DeserializeBoxed + Send + 'static {
    type Response = T::Result;
}

#[derive(Debug, Clone)]
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

impl<R> SerializeBoxed for WaitSeqno<R> where R: Requestable {
    fn serialize_boxed(&self, se: &mut Serializer) {
        self.prefix.serialize_boxed(se);
        self.request.serialize_boxed(se);
    }
}

impl<R> Requestable for WaitSeqno<R> where R: Requestable {
    type Response = R::Response;
}
