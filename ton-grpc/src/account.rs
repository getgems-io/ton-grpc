use tonic::{async_trait, Request, Response, Status};
use tonlibjson_client::ton::TonClient;
use anyhow::{anyhow, Result};
use base64::Engine;
use tonlibjson_client::block::RawFullAccountState;
use crate::ton::account_server::Account;
use crate::ton::{AccountAddress, AccountState, AccountStateOnBlock, FullAccountState, GetAccountStateRequest, TvmCell};
use crate::ton::account_state_on_block::AccountState;

pub struct AccountService {
    client: TonClient
}

impl AccountService {
    pub async fn from_env() -> Result<Self> {
        Ok(Self {
            client: TonClient::from_env().await?
        })
    }
}

impl TryFrom<AccountAddress> for tonlibjson_client::block::AccountAddress {
    type Error = anyhow::Error;

    fn try_from(value: AccountAddress) -> Result<Self, Self::Error> {
        Self::new(&value.address)
    }
}

impl TryFrom<RawFullAccountState> for AccountStateOnBlock {
    type Error = anyhow::Error;

    fn try_from(value: RawFullAccountState) -> Result<Self, Self::Error> {


        Ok(Self {
            balance: value.balance.unwrap_or_default(),
            block_id: value.block_id.into(),
            last_transaction_id: value.last_transaction_id.into(),
        })
    }
}

#[async_trait]
impl Account for AccountService {
    async fn get_account_state(&self, request: Request<GetAccountStateRequest>) -> std::result::Result<Response<AccountStateOnBlock>, Status> {
        let msg = request.into_inner();
        let block = match msg.block {
            Some(block) => block,
            None => self.client.get_masterchain_info().await?.last
        };
        let address = msg.account_address
            .ok_or_else(|| Err(anyhow!("Account address is empty")))?;

        let state = self.client.raw_get_account_state(&address.address).await?;

        let balance = state.balance.unwrap_or_default();
        let last_transaction_id = state.last_transaction_id.into();
        let state: AccountState = state.into();

        Ok(Response::new(AccountStateOnBlock {
            balance,
            account_address: Some(address),
            block_id: Some(block),
            last_transaction_id,
            account_state: Some(state)
        }))
    }

    async fn get_shard_account_cell(&self, request: Request<AccountAddress>) -> Result<Response<TvmCell>, Status> {
        let msg = request.into_inner();

        let address: tonlibjson_client::block::AccountAddress = msg.try_into()
            .map_err(|e: anyhow::Error| Status::internal(e.to_string()))?;

        let response = self.client
            .get_shard_account_cell(&address.account_address)
            .await
            .map_err(|e| Status::internal(e.to_string()))?;

        let bytes = base64::engine::general_purpose::STANDARD
            .decode(response.bytes)
            .map_err(|e| Status::internal(e.to_string()))?;

        Ok(Response::new(TvmCell { bytes }))
    }
}
