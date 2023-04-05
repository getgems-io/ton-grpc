use tonic::{async_trait, Request, Response, Status};
use tonlibjson_client::ton::TonClient;
use anyhow::Result;
use base64::Engine;
use crate::ton::account_server::Account;
use crate::ton::{AccountAddress, TvmCell};

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

    fn try_from(value: AccountAddress) -> std::result::Result<Self, Self::Error> {
        Self::new(value.address)
    }
}

#[async_trait]
impl Account for AccountService {
    async fn get_shard_account_cell(&self, request: Request<AccountAddress>) -> Result<Response<TvmCell>, Status> {
        let msg = request.into_inner();

        let address: tonlibjson_client::block::AccountAddress = msg.try_into()
            .map_err(|e: anyhow::Error| Status::internal(e.to_string()))?;

        let response = self.client
            .get_shard_account_cell(address)
            .await
            .map_err(|e| Status::internal(e.to_string()))?;

        let bytes = base64::engine::general_purpose::STANDARD
            .decode(response.bytes)
            .map_err(|e| Status::internal(e.to_string()))?;

        Ok(Response::new(TvmCell { bytes }))
    }
}
