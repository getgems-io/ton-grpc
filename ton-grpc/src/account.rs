use tonic::{async_trait, Request, Response, Status};
use tonlibjson_client::ton::TonClient;
use anyhow::Result;
use base64::Engine;
use crate::ton::account_server::Account;
use crate::ton::{GetAccountStateRequest, GetAccountStateResponse, GetShardAccountCellRequest, GetShardAccountCellResponse, TvmCell};
use crate::ton::get_account_state_request::Criteria;
use crate::ton::get_account_state_response::AccountState;

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

#[async_trait]
impl Account for AccountService {
    async fn get_account_state(&self, request: Request<GetAccountStateRequest>) -> std::result::Result<Response<GetAccountStateResponse>, Status> {
        let msg = request.into_inner();

        let address = msg.account_address
            .ok_or_else(|| Status::invalid_argument("Empty AccountAddress"))?;

        let criteria = match msg.criteria {
            None => {
                let block_id = self.client.get_masterchain_info()
                    .await
                    .map(|i| i.last);

                either::Left(block_id)
            },
            Some(Criteria::BlockId(block_id)) => either::Left(block_id.try_into()),
            Some(Criteria::TransactionId(tx_id)) => either::Right(tx_id.try_into())
        }.factor_err()
            .map_err(|e| Status::internal(e.to_string()))?;

        let state = criteria.map_left(|block_id| async {
            self.client.raw_get_account_state_on_block(&address.address, block_id)
                .await
        }).map_right(|tx_id| async {
            self.client.raw_get_account_state_by_transaction(&address.address, tx_id)
                .await
        }).await.map_err(|e| Status::internal(e.to_string()))?;

        let block_id = state.block_id.clone();
        let balance = state.balance.unwrap_or_default();
        let last_transaction_id = state.last_transaction_id.clone().map(|t| t.into());
        let state: AccountState = state.into();

        Ok(Response::new(GetAccountStateResponse {
            balance,
            account_address: Some(address),
            block_id: Some(block_id.into()),
            last_transaction_id,
            account_state: Some(state)
        }))
    }

    async fn get_shard_account_cell(&self, request: Request<GetShardAccountCellRequest>) -> Result<Response<GetShardAccountCellResponse>, Status> {
        let msg = request.into_inner();

        let address = msg.account_address
            .ok_or_else(|| Status::invalid_argument("Empty AccountAddress"))?;

        let block_id = match msg.block_id {
            Some(block) => block.try_into()
                .map_err(|e: anyhow::Error| Status::internal(e.to_string()))?,
            None => self.client.get_masterchain_info().await
                .map_err(|e| Status::internal(e.to_string()))?
                .last
        };

        let response = self.client
            .get_shard_account_cell_on_block(&address.address, block_id.clone())
            .await
            .map_err(|e| Status::internal(e.to_string()))?;

        let bytes = base64::engine::general_purpose::STANDARD
            .decode(response.bytes)
            .map_err(|e| Status::internal(e.to_string()))?;

        let response = GetShardAccountCellResponse {
            account_address: Some(address),
            block_id: Some(block_id.into()),
            cell: Some(TvmCell { bytes })
        };

        Ok(Response::new(response))
    }
}
