use crate::TonContractError;
use std::str::FromStr;
use ton_address::SmartContractAddress;
use ton_client::Client;
use ton_client::TonService;
use ton_tower::response::StackEntry;
use toner::ton::MsgAddress;

pub struct TonContract<S> {
    address: MsgAddress,
    client: Client<S>,
}

impl<S: TonService> TonContract<S> {
    pub fn new(client: Client<S>, address: MsgAddress) -> Self {
        Self { client, address }
    }

    pub fn address(&self) -> MsgAddress {
        self.address
    }

    pub fn client(&self) -> Client<S> {
        self.client.clone()
    }

    pub async fn run_get_method(
        &self,
        method: impl AsRef<str>,
        stack: Vec<StackEntry>,
    ) -> Result<Vec<StackEntry>, TonContractError> {
        let mut client = self.client.clone();
        let result = client
            .run_get_method(
                &SmartContractAddress::from_str(&self.address().to_hex())?,
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
