use std::pin::Pin;
use std::str::FromStr;
use tonic::{async_trait, Request, Response, Status};
use tonlibjson_client::ton::TonClient;
use anyhow::Result;
use futures::{Stream, StreamExt, try_join, TryStreamExt};
use derive_new::new;
use tonlibjson_client::address::AccountAddressData;
use crate::helpers::{extend_block_id, extend_from_tx_id, extend_to_tx_id};
use crate::ton::account_service_server::AccountService as BaseAccountService;
use crate::ton::{GetAccountStateRequest, GetAccountStateResponse, GetAccountTransactionsRequest, GetShardAccountCellRequest, GetShardAccountCellResponse, Transaction};
use crate::ton::get_account_state_response::AccountState;
use crate::ton::{get_account_state_request, get_shard_account_cell_request};
use crate::ton::get_account_transactions_request::Order;

#[derive(new)]
pub struct AccountService {
    client: TonClient
}

#[async_trait]
impl BaseAccountService for AccountService {
    #[tracing::instrument(skip_all, err)]
    async fn get_account_state(&self, request: Request<GetAccountStateRequest>) -> std::result::Result<Response<GetAccountStateResponse>, Status> {
        let msg = request.into_inner();

        let address = AccountAddressData::from_str(&msg.account_address)
            .map_err(|e| Status::internal(e.to_string()))?;

        let criteria = match msg.criteria {
            None => {
                let block_id = self.client.get_masterchain_info()
                    .await
                    .map(|i| i.last);

                either::Left(block_id)
            },
            Some(get_account_state_request::Criteria::BlockId(block_id)) => {
                either::Left(extend_block_id(&self.client, &block_id).await)
            },
            Some(get_account_state_request::Criteria::TransactionId(tx_id)) => either::Right(Ok(tx_id.into()))
        }.factor_err().map_err(|e| Status::internal(e.to_string()))?;

        let state = criteria.map_left(|block_id| async {
            self.client.raw_get_account_state_on_block(&msg.account_address, block_id)
                .await
        }).map_right(|tx_id| async {
            self.client.raw_get_account_state_by_transaction(&msg.account_address, tx_id)
                .await
        }).await.map_err(|e| Status::internal(e.to_string()))?;

        let block_id = state.block_id.clone();
        let balance = state.balance.unwrap_or_default();
        let last_transaction_id = state.last_transaction_id.clone().map(|t| (&address, t).into());
        let state: AccountState = state.into();
        let block_id = block_id.into();

        Ok(Response::new(GetAccountStateResponse {
            balance,
            account_address: msg.account_address,
            block_id: Some(block_id),
            last_transaction_id,
            account_state: Some(state)
        }))
    }

    #[tracing::instrument(skip_all, err)]
    async fn get_shard_account_cell(&self, request: Request<GetShardAccountCellRequest>) -> Result<Response<GetShardAccountCellResponse>, Status> {
        let msg = request.into_inner();

        let criteria = match msg.criteria {
            None => {
                let block_id = self.client.get_masterchain_info()
                    .await
                    .map(|i| i.last);

                either::Left(block_id)
            },
            Some(get_shard_account_cell_request::Criteria::BlockId(block_id)) => {
                either::Left(extend_block_id(&self.client, &block_id).await)
            },
            Some(get_shard_account_cell_request::Criteria::TransactionId(tx_id)) => either::Right(Ok(tx_id.into()))
        }.factor_err().map_err(|e| Status::internal(e.to_string()))?;

        let (block_id, cell) = criteria.map_left(|block_id| async {
            let cell = self.client.get_shard_account_cell_on_block(&msg.account_address, block_id.clone()).await?;

            Ok((block_id, cell))
        }).map_right(|tx_id| async {
            let state = self.client.raw_get_account_state_by_transaction(&msg.account_address, tx_id).await?;
            let cell = self.client.get_shard_account_cell_on_block(&msg.account_address, state.block_id.clone()).await?;

            Ok((state.block_id, cell))
        }).await.map_err(|e: anyhow::Error| Status::internal(e.to_string()))?;

        let block_id = block_id.into();
        let cell = cell.into();

        let response = GetShardAccountCellResponse {
            account_address: msg.account_address,
            block_id: Some(block_id),
            cell: Some(cell)
        };

        Ok(Response::new(response))
    }

    type GetAccountTransactionsStream = Pin<Box<dyn Stream<Item=Result<Transaction, Status>> + Send + 'static>>;

    #[tracing::instrument(skip_all, err)]
    async fn get_account_transactions(&self, request: Request<GetAccountTransactionsRequest>) -> std::result::Result<Response<Self::GetAccountTransactionsStream>, Status> {
        let msg = request.into_inner();
        let client = self.client.clone();

        let address = AccountAddressData::from_str(&msg.account_address)
            .map_err(|e| Status::internal(e.to_string()))?;

        let (from_tx, to_tx) = try_join!(
            extend_from_tx_id(&client, &msg.account_address, msg.from.clone()),
            extend_to_tx_id(&client, &msg.account_address, msg.to.clone())
        ).map_err(|e: anyhow::Error| Status::internal(e.to_string()))?;

        let stream = match msg.order() {
            Order::Unordered => {
                client.get_account_tx_range_unordered(&msg.account_address, (from_tx, to_tx)).await
                    .map_err(|e: anyhow::Error| Status::internal(e.to_string()))?
                    .boxed()
            },
            Order::FromNewToOld => {
                client.get_account_tx_range(&msg.account_address, (from_tx, to_tx)).boxed()
            }
        }
            .map_ok(move |t| (&address, t).into())
            .map_err(|e: anyhow::Error| Status::internal(e.to_string()))
            .boxed();

        Ok(Response::new(stream))
    }
}
