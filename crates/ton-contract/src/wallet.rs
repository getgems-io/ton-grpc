use async_trait::async_trait;

use crate::{adapters::TvmBoxedStackEntryExt, TonContract, TonContractError};

#[async_trait]
pub trait WalletContract {
    async fn seqno(&self) -> Result<u32, TonContractError>;
}

#[async_trait]
impl WalletContract for TonContract {
    async fn seqno(&self) -> Result<u32, TonContractError> {
        let [seqno] = self.run_get_method("seqno", [].into()).await?.try_into()?;
        seqno.to_number()
    }
}
