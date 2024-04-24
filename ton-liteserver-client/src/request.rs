use tl_core::deserializer::Deserialize;
use tl_core::serializer::Serialize;
use tl_core::types::Functional;

pub trait Requestable: Serialize + Send {
    type Response: Deserialize + Send + 'static;
}

impl<T> Requestable for T
    where T : Functional + Serialize + Send,
        T::Result: Deserialize + Send + 'static
{
    type Response = T::Result;
}
