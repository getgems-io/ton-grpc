use crate::boxed::Boxed;

pub trait Functional {
    type Result;
}

pub trait BareType where Self: Sized {
    const CONSTRUCTOR_NUMBER_BE: u32;

    fn into_boxed(self) -> Boxed<Self> {
        Boxed::new(self)
    }
}

pub trait BoxedType where Self: Sized {
    fn constructor_number(&self) -> u32;
}

// TODO[akostylev0] review
pub type Double = f64;
pub type Int31 = i32; // "#" / nat type
pub type Int32 = i32;
pub type Int = i32;
pub type Int53 = i64;
pub type Int64 = i64;
pub type Long = i64;
pub type Int128 = i128;
pub type Int256 = [u8; 32];
pub type BoxedBool = bool;
pub type Bytes = Vec<u8>;
pub type String = Vec<u8>;
pub type Object = Bytes;
pub type SecureString = String;
pub type SecureBytes = Vec<u8>;
pub type Vector<T> = Vec<T>;