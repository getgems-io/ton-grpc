use adnl_tcp::deserializer::Deserialize;
use adnl_tcp::serializer::Serialize;
use crate::tl::Functional;

pub trait Requestable: Serialize + Send {
    type Response: Deserialize + Send + 'static;
}

impl<T> Requestable for T
    where T : Functional + Serialize + Send,
        T::Result: Deserialize + Send + 'static
{
    type Response = T::Result;
}