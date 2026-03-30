use thiserror::Error as ThisError;
use toner::tlb::StringError as TlbError;
use ton_client::StackEntry;

#[derive(Debug, ThisError)]
pub enum TonContractError {
    #[error("contract failed with exit code: {0}")]
    Contract(i32),
    #[error("invalid output stack")]
    InvalidStack,
    #[error("TLB: {0}")]
    TLB(#[from] TlbError),
    #[error("base64: {0}")]
    Base64(#[from] base64::DecodeError),
    #[error("cannot parse number: {0}")]
    ParseNumber(String),
    #[error(transparent)]
    Client(#[from] anyhow::Error),
}

impl From<Vec<StackEntry>> for TonContractError {
    fn from(_: Vec<StackEntry>) -> Self {
        Self::InvalidStack
    }
}
