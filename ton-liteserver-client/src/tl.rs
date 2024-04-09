pub trait Functional {
    type Result;
}

type Double = f64;
type Int31 = i32; // "#" / nat type
type Int32 = i32;
type Int = i32;
type Int53 = i64;
type Int64 = i64;
type Long = i64;
type Int128 = i128;
type Int256 = String;
type BoxedBool = bool;
type Bytes = Vec<u8>;
type SecureString = String;
type SecureBytes = Vec<u8>;
type Vector<T> = Vec<T>;

include!(concat!(env!("OUT_DIR"), "/generated.rs"));
