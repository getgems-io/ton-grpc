pub mod v3r2;

#[cfg(test)]
mod integration;

use crate::{TonContract, TonContractError, adapters::StackEntryExt};
use async_trait::async_trait;
use ton_client::TonService;

#[async_trait]
pub trait WalletContract {
    async fn seqno(&self) -> Result<u32, TonContractError>;
}

#[async_trait]
impl<S: TonService> WalletContract for TonContract<S> {
    async fn seqno(&self) -> Result<u32, TonContractError> {
        let [seqno] = self.run_get_method("seqno", [].into()).await?.try_into()?;
        seqno.to_number()
    }
}
