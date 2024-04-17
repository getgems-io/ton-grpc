use crate::deserializer::{Deserialize, Deserializer};
use crate::serializer::{Serialize, Serializer};
use crate::types::{BareType, BoxedType, Functional};

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

impl<T> Functional for Boxed<T> where T: Functional + BareType, T::Result : BoxedType {
    type Result = T::Result;
}

impl<T> BoxedType for Boxed<T> where T : BareType {
    fn constructor_number(&self) -> u32 {
        T::CONSTRUCTOR_NUMBER_BE
    }
}

impl<T> Serialize for Boxed<T> where T : BareType + Serialize {
    fn serialize(&self, se: &mut Serializer) -> anyhow::Result<()> {
        se.write_constructor_number(self.constructor_number());

        self.inner.serialize(se)
    }
}

impl<T> Deserialize for Boxed<T> where T : BareType + Deserialize {
    fn deserialize(de: &mut Deserializer) -> anyhow::Result<Self> {
        de.verify_constructor_number(T::CONSTRUCTOR_NUMBER_BE)?;

        Ok(Boxed { inner: T::deserialize(de)? })
    }
}
