use adnl_tcp::deserializer::DeserializeBoxed;
use adnl_tcp::serializer::SerializeBoxed;
use adnl_tcp::types::Functional;

pub trait Requestable: SerializeBoxed + Send {
    type Response: DeserializeBoxed + Send + 'static;
}

impl<T> Requestable for T
    where T : Functional + SerializeBoxed + Send,
        T::Result: DeserializeBoxed + Send + 'static {
    type Response = T::Result;
}

pub struct WithSeqno<R> {
    request: R,
    seqno: i32
}

impl<R> WithSeqno<R> where R: Requestable + Sized {
    pub fn new(request: R, seqno: i32) -> Self {
        Self { request, seqno }
    }
}
