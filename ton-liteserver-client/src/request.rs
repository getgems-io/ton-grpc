use crate::deserializer::Deserialize;
use crate::serializer::Serialize;
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