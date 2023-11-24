use std::pin::Pin;
use std::str::FromStr;
use tonic::{async_trait, Request, Response, Status};
use tonlibjson_client::ton::TonClient;
use anyhow::Result;
use futures::{Stream, StreamExt, try_join, TryStreamExt, TryFutureExt};
use derive_new::new;
use tonlibjson_client::address::AccountAddressData;
use tonlibjson_client::block::{RawFullAccountState, TonBlockIdExt, TvmCell};
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

        let state = self.fetch_account_state(&msg)
            .map_err(|e| Status::internal(e.to_string()))
            .await?;

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

        let (block_id, cell) = self.fetch_shard_account_cell(&msg)
            .await
            .map_err(|e| Status::internal(e.to_string()))?;

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
                client.get_account_tx_range_unordered(&msg.account_address, (from_tx, to_tx))
                    .await
                    .map_err(|e: anyhow::Error| Status::internal(e.to_string()))?
                    .boxed()
            },
            Order::FromNewToOld => {
                client.get_account_tx_range(&msg.account_address, (from_tx, to_tx))
                    .boxed()
            }
        }
            .map_ok(move |t| (&address, t).into())
            .map_err(|e: anyhow::Error| {
                tracing::error!(error = %e, "get_account_transactions failed");
                Status::internal(e.to_string())
            })
            .boxed();

        Ok(Response::new(stream))
    }
}

impl AccountService {
    async fn fetch_account_state(&self, msg: &GetAccountStateRequest) -> Result<RawFullAccountState> {
        let state = match &msg.criteria {
            None => {
                let block_id = self.client.get_masterchain_info().await?.last;

                self.client.raw_get_account_state_at_least_block(&msg.account_address, &block_id).await?
            },
            Some(get_account_state_request::Criteria::BlockId(block_id)) => {
                let block_id = extend_block_id(&self.client, block_id).await?;

                self.client.raw_get_account_state_on_block(&msg.account_address, block_id).await?
            },
            Some(get_account_state_request::Criteria::AtLeastBlockId(block_id)) => {
                let block_id = extend_block_id(&self.client, block_id).await?;

                self.client.raw_get_account_state_at_least_block(&msg.account_address, &block_id).await?
            },
            Some(get_account_state_request::Criteria::TransactionId(tx_id)) => {
                self.client.raw_get_account_state_by_transaction(&msg.account_address, tx_id.clone().into()).await?
            },
        };
        Ok(state)
    }

    async fn fetch_shard_account_cell(&self, msg: &GetShardAccountCellRequest) -> Result<(TonBlockIdExt, TvmCell)> {
        let (block_id, cell) = match &msg.criteria {
            None => {
                let block_id = self.client.get_masterchain_info().await?.last;
                let cell = self.client.get_shard_account_cell_at_least_block(&msg.account_address, &block_id).await?;

                (block_id, cell)
            }
            Some(get_shard_account_cell_request::Criteria::BlockId(block_id)) => {
                let block_id = extend_block_id(&self.client, block_id).await?;
                let cell = self.client.get_shard_account_cell_on_block(&msg.account_address, block_id.clone()).await?;

                (block_id, cell)
            },
            Some(get_shard_account_cell_request::Criteria::TransactionId(tx_id)) => {
                let state = self.client.raw_get_account_state_by_transaction(&msg.account_address, tx_id.clone().into()).await?;
                let cell = self.client.get_shard_account_cell_on_block(&msg.account_address, state.block_id.clone()).await?;

                (state.block_id, cell)
            },
            Some(get_shard_account_cell_request::Criteria::AtLeastBlockId(block_id)) => {
                let block_id = extend_block_id(&self.client, block_id).await?;
                let state = self.client.raw_get_account_state_at_least_block(&msg.account_address, &block_id).await?;
                let cell = self.client.get_shard_account_cell_on_block(&msg.account_address, state.block_id.clone()).await?;

                (state.block_id, cell)
            }
        };

        Ok((block_id, cell))
    }
}

#[cfg(test)]
mod tests {
    use futures::StreamExt;
    use tonic::Request;
    use tonlibjson_client::ton::TonClientBuilder;
    use tracing_test::traced_test;
    use crate::account::AccountService;
    use crate::ton::account_service_server::AccountService as BaseAccountService;
    use crate::ton::{get_account_transactions_request, GetAccountStateRequest, GetAccountTransactionsRequest, GetShardAccountCellRequest, PartialTransactionId};
    use crate::ton::get_account_transactions_request::bound;

    #[tokio::test]
    #[traced_test]
    async fn account_get_from_to() {
        tracing::info!("prep client");
        let mut client = TonClientBuilder::default().await.unwrap();
        client.ready().await.unwrap();
        tracing::info!("ready");
        let svc = AccountService::new(client);
        let req = Request::new(GetAccountTransactionsRequest {
            account_address: "EQCkgtq1pKJh4Zpif_z4RR2aYmespuImTw15amEacGX-k6Zj".to_string(),
            order: 1,
            from: Some(get_account_transactions_request::Bound { r#type: 0, bound: Some(bound::Bound::TransactionId(PartialTransactionId {
                lt: 42048922000003,
                hash: "JatZ7mIBIfBpCNHHHQkpIc1+72RrzSiM8xvqlqRAbmc=".to_string()
            }))}),
            to: Some(get_account_transactions_request::Bound { r#type: 0, bound: Some(bound::Bound::TransactionId(PartialTransactionId {
                lt: 42048922000003,
                hash: "JatZ7mIBIfBpCNHHHQkpIc1+72RrzSiM8xvqlqRAbmc=".to_string()
            }))})
        });

        let resp = svc.get_account_transactions(req).await.unwrap();

        let txs: Vec<_> = resp.into_inner().collect::<Vec<_>>().await;
        tracing::info!("got txs: {:?}", txs);
        assert_eq!(1, txs.len())
    }

    #[tokio::test]
    #[traced_test]
    async fn get_account_state_without_criteria() {
        tracing::info!("prep client");
        let mut client = TonClientBuilder::default().await.unwrap();
        client.ready().await.unwrap();
        tracing::info!("ready");
        let svc = AccountService::new(client);
        let req = Request::new(GetAccountStateRequest {
            account_address: "EQCaatdRleXHdMCc3ONQsZklcF32jyCiJhHyN3YEKxPXMhsF".to_string(),
            criteria: None
        });

        let resp = svc.get_account_state(req).await;

        tracing::info!(resp = ?resp);
        assert!(resp.is_ok())
    }

    #[tokio::test]
    #[traced_test]
    async fn get_shard_account_cell_without_criteria() {
        tracing::info!("prep client");
        let mut client = TonClientBuilder::default().await.unwrap();
        client.ready().await.unwrap();
        tracing::info!("ready");
        let svc = AccountService::new(client);
        let req = Request::new(GetShardAccountCellRequest {
            account_address: "EQCaatdRleXHdMCc3ONQsZklcF32jyCiJhHyN3YEKxPXMhsF".to_string(),
            criteria: None
        });

        let resp = svc.get_shard_account_cell(req).await;

        tracing::info!(resp = ?resp);
        assert!(resp.is_ok())
    }
}
