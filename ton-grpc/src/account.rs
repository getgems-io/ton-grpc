use tonic::{async_trait, Request, Response, Status};
use tonlibjson_client::ton::TonClient;
use anyhow::Result;
use base64::Engine;
use crate::ton::account_server::Account;
use crate::ton::{GetAccountStateRequest, GetAccountStateResponse, GetShardAccountCellRequest, GetShardAccountCellResponse, TvmCell};
use crate::ton::get_account_state_response::AccountState;
use crate::ton::{get_account_state_request, get_shard_account_cell_request};

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

// TODO[akostylev0] add actual block_id to response

#[async_trait]
impl Account for AccountService {
    #[tracing::instrument(skip_all)]
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
            Some(get_account_state_request::Criteria::BlockId(block_id)) => either::Left(block_id.try_into()),
            Some(get_account_state_request::Criteria::TransactionId(tx_id)) => either::Right(tx_id.try_into())
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

    #[tracing::instrument(skip_all)]
    async fn get_shard_account_cell(&self, request: Request<GetShardAccountCellRequest>) -> Result<Response<GetShardAccountCellResponse>, Status> {
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
            Some(get_shard_account_cell_request::Criteria::BlockId(block_id)) => either::Left(block_id.try_into()),
            Some(get_shard_account_cell_request::Criteria::TransactionId(tx_id)) => either::Right(tx_id.try_into())
        }.factor_err()
            .map_err(|e| Status::internal(e.to_string()))?;

        let (block_id, cell) = criteria.map_left(|block_id| async {
            let cell = self.client.get_shard_account_cell_on_block(&address.address, block_id.clone()).await?;

            Ok((block_id, cell))
        }).map_right(|tx_id| async {
            let state = self.client.raw_get_account_state_by_transaction(&address.address, tx_id).await?;
            let cell = self.client.get_shard_account_cell_on_block(&address.address, state.block_id.clone()).await?;

            Ok((state.block_id, cell))
        }).await.map_err(|e: anyhow::Error| Status::internal(e.to_string()))?;

        let bytes = base64::engine::general_purpose::STANDARD
            .decode(cell.bytes)
            .map_err(|e| Status::internal(e.to_string()))?;

        let response = GetShardAccountCellResponse {
            account_address: Some(address),
            block_id: Some(block_id.into()),
            cell: Some(TvmCell { bytes })
        };

        Ok(Response::new(response))
    }
}
