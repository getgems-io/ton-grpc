use adnl_tcp::deserializer::DeserializeBoxed;
use adnl_tcp::serializer::SerializeBoxed;
use adnl_tcp::types::Functional;

pub trait Requestable: SerializeBoxed + Send {
    type Response: DeserializeBoxed + Send + 'static;
}

impl<T> Requestable for T
    where T : Functional + SerializeBoxed + Send,
        T::Result: DeserializeBoxed + Send + 'static
{
    type Response = T::Result;
}


pub struct WithMasterChainSeqno<R> {
    pub inner: R,
    pub seqno: i32,
}
