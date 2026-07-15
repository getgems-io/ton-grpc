#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("input contains an interior NUL byte")]
    InvalidCString(#[from] std::ffi::NulError),

    #[error("emulator FFI call failed")]
    Ffi,

    #[error("native emulator returned invalid UTF-8")]
    InvalidUtf8(#[from] std::str::Utf8Error),

    #[error("params BoC is too large: {len} bytes")]
    ParamsBocTooLarge { len: usize },
}

pub type Result<T> = std::result::Result<T, Error>;
