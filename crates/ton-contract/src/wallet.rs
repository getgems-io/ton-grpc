use async_trait::async_trait;
use ton_client::TonClient;

use crate::{TonContract, TonContractError, adapters::StackEntryExt};

#[async_trait]
pub trait WalletContract {
    async fn seqno(&self) -> Result<u32, TonContractError>;
}

#[async_trait]
impl<T: TonClient> WalletContract for TonContract<T> {
    async fn seqno(&self) -> Result<u32, TonContractError> {
        let [seqno] = self.run_get_method("seqno", [].into()).await?.try_into()?;
        seqno.to_number()
    }
}
