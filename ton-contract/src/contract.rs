use toner::tlb::ton::MsgAddress;
use tonlibjson_client::{
    block::{SmcRunResult, TvmBoxedStackEntry},
    ton::TonClient,
};

use crate::TonContractError;

pub struct TonContract {
    address: MsgAddress,
    client: TonClient,
}

impl TonContract {
    pub fn new(client: TonClient, address: MsgAddress) -> Self {
        Self { client, address }
    }

    pub fn address(&self) -> MsgAddress {
        self.address
    }

    pub fn client(&self) -> TonClient {
        self.client.clone()
    }

    pub async fn run_get_method(
        &self,
        method: impl AsRef<str>,
        stack: Vec<TvmBoxedStackEntry>,
    ) -> Result<Vec<TvmBoxedStackEntry>, TonContractError> {
        let SmcRunResult {
            stack, exit_code, ..
        } = self
            .client
            .run_get_method(
                self.address().to_base64_std(),
                method.as_ref().to_string(),
                stack,
            )
            .await?;
        Ok(match exit_code {
            0 | 1 => stack,
            _ => return Err(TonContractError::Contract(exit_code)),
        })
    }
}
