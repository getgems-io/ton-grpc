use crate::deserializer::{Deserialize, Deserializer};
use crate::serializer::{Serialize, Serializer};
use crate::types::BareType;

#[derive(Debug, Eq, PartialEq, Clone)]
pub struct Boxed<T> where T: BareType {
    inner: T
}

impl<T> Boxed<T> where T: BareType {
    pub fn new(inner: T) -> Self {
        Self { inner }
    }

    pub fn unbox(self) -> T {
        self.inner
    }
}

impl<T> Serialize for Boxed<T> where T : BareType + Serialize {
    fn serialize(&self, se: &mut Serializer) {
        se.write_constructor_number(T::CONSTRUCTOR_NUMBER_BE);

        self.inner.serialize(se);
    }
}

impl<T> Deserialize for Boxed<T> where T : BareType + Deserialize {
    fn deserialize(de: &mut Deserializer) -> anyhow::Result<Self> {
        de.verify_constructor_number(T::CONSTRUCTOR_NUMBER_BE)?;

        Ok(Boxed { inner: T::deserialize(de)? })
    }
}
