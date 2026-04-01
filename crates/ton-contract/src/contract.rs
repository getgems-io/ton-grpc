use crate::TonContractError;
use std::str::FromStr;
use ton_address::SmartContractAddress;
use ton_client::{StackEntry, TonClient};
use toner::ton::MsgAddress;

pub struct TonContract<T> {
    address: MsgAddress,
    client: T,
}

impl<T: TonClient> TonContract<T> {
    pub fn new(client: T, address: MsgAddress) -> Self {
        Self { client, address }
    }

    pub fn address(&self) -> MsgAddress {
        self.address
    }

    pub fn client(&self) -> T {
        self.client.clone()
    }

    pub async fn run_get_method(
        &self,
        method: impl AsRef<str>,
        stack: Vec<StackEntry>,
    ) -> Result<Vec<StackEntry>, TonContractError> {
        let result = self
            .client
            .run_get_method(
                &SmartContractAddress::from_str(&self.address().to_base64_std())?,
                method.as_ref(),
                stack,
            )
            .await?;
        Ok(match result.exit_code {
            0 | 1 => result.stack,
            _ => return Err(TonContractError::Contract(result.exit_code)),
        })
    }
}
