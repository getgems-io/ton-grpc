#![allow(clippy::blocks_in_conditions)]

use crate::helpers::{extend_block_id, extend_from_tx_id, extend_to_tx_id};
use crate::ton::account_service_server::AccountService as BaseAccountService;
use crate::ton::get_account_state_response::AccountState;
use crate::ton::get_account_transactions_request::Order;
use crate::ton::{
    GetAccountStateRequest, GetAccountStateResponse, GetAccountTransactionsRequest,
    GetShardAccountCellRequest, GetShardAccountCellResponse, Transaction,
};
use crate::ton::{get_account_state_request, get_shard_account_cell_request};
use anyhow::Result;
use derive_new::new;
use futures::{Stream, StreamExt, TryFutureExt, TryStreamExt, try_join};
use std::pin::Pin;
use std::str::FromStr;
use ton_address::SmartContractAddress;
use ton_client::{AccountClientExt as _, TonClient};
use tonic::{Request, Response, Status, async_trait};

#[derive(new)]
pub struct AccountService<T: TonClient> {
    client: T,
}

#[async_trait]
impl<T: TonClient> BaseAccountService for AccountService<T> {
    #[tracing::instrument(skip_all, err)]
    async fn get_account_state(
        &self,
        request: Request<GetAccountStateRequest>,
    ) -> std::result::Result<Response<GetAccountStateResponse>, Status> {
        let msg = request.into_inner();

        let state = self
            .fetch_account_state(&msg)
            .map_err(|e| Status::internal(e.to_string()))
            .await?;

        let balance = state.balance.unwrap_or_default();
        let last_transaction_id =
            state
                .last_transaction_id
                .clone()
                .map(|t| crate::ton::TransactionId {
                    account_address: msg.account_address.clone(),
                    lt: t.lt,
                    hash: t.hash,
                });
        let account_state: AccountState = state.clone().into();
        let block_id = state.block_id.into();

        Ok(Response::new(GetAccountStateResponse {
            balance,
            account_address: msg.account_address,
            block_id: Some(block_id),
            last_transaction_id,
            account_state: Some(account_state),
        }))
    }

    #[tracing::instrument(skip_all, err)]
    async fn get_shard_account_cell(
        &self,
        request: Request<GetShardAccountCellRequest>,
    ) -> Result<Response<GetShardAccountCellResponse>, Status> {
        let msg = request.into_inner();

        let (block_id, cell) = self
            .fetch_shard_account_cell(&msg)
            .await
            .map_err(|e| Status::internal(e.to_string()))?;

        let response = GetShardAccountCellResponse {
            account_address: msg.account_address,
            block_id: Some(block_id.into()),
            cell: Some(cell.into()),
        };

        Ok(Response::new(response))
    }

    type GetAccountTransactionsStream =
        Pin<Box<dyn Stream<Item = Result<Transaction, Status>> + Send + 'static>>;

    #[tracing::instrument(skip_all, err)]
    async fn get_account_transactions(
        &self,
        request: Request<GetAccountTransactionsRequest>,
    ) -> std::result::Result<Response<Self::GetAccountTransactionsStream>, Status> {
        let msg = request.into_inner();
        let account_address = SmartContractAddress::from_str(&msg.account_address)
            .map_err(|e| Status::invalid_argument(e.to_string()))?;

        let client = self.client.clone();

        let (from_tx, to_tx) = try_join!(
            extend_from_tx_id(&client, &account_address, msg.from.clone()),
            extend_to_tx_id(&client, &account_address, msg.to.clone())
        )
        .map_err(|e: anyhow::Error| Status::internal(e.to_string()))?;

        let stream = match msg.order() {
            Order::Unordered => client
                .get_account_tx_range_unordered(&account_address, (from_tx, to_tx))
                .await
                .map_err(|e: anyhow::Error| Status::internal(e.to_string()))?
                .boxed(),
            Order::FromNewToOld => client
                .get_account_tx_range(&account_address, (from_tx, to_tx))
                .boxed(),
        }
        .map_ok(move |t| t.into())
        .map_err(|e: anyhow::Error| {
            tracing::error!(error = %e, "get_account_transactions failed");
            Status::internal(e.to_string())
        })
        .boxed();

        Ok(Response::new(stream))
    }
}

impl<T: TonClient> AccountService<T> {
    async fn fetch_account_state(
        &self,
        msg: &GetAccountStateRequest,
    ) -> Result<ton_client::AccountState> {
        let account_address = SmartContractAddress::from_str(&msg.account_address)
            .map_err(|e| Status::invalid_argument(e.to_string()))?;

        let state = match &msg.criteria {
            None => {
                let block_id = self.client.get_masterchain_info().await?.last;

                self.client
                    .get_account_state_at_least_block(&account_address, &block_id)
                    .await?
            }
            Some(get_account_state_request::Criteria::BlockId(block_id)) => {
                let block_id = extend_block_id(&self.client, block_id).await?;

                self.client
                    .get_account_state_on_block(&account_address, block_id)
                    .await?
            }
            Some(get_account_state_request::Criteria::AtLeastBlockId(block_id)) => {
                let block_id = extend_block_id(&self.client, block_id).await?;

                self.client
                    .get_account_state_at_least_block(&account_address, &block_id)
                    .await?
            }
            Some(get_account_state_request::Criteria::TransactionId(tx_id)) => {
                self.client
                    .get_account_state_by_transaction(&account_address, tx_id.clone().into())
                    .await?
            }
        };
        Ok(state)
    }

    async fn fetch_shard_account_cell(
        &self,
        msg: &GetShardAccountCellRequest,
    ) -> Result<(ton_client::BlockIdExt, ton_client::Cell)> {
        let account_address = SmartContractAddress::from_str(&msg.account_address)
            .map_err(|e| Status::invalid_argument(e.to_string()))?;

        let (block_id, cell) = match &msg.criteria {
            None => {
                let block_id = self.client.get_masterchain_info().await?.last;
                let cell = self
                    .client
                    .get_shard_account_cell_at_least_block(&account_address, &block_id)
                    .await?;

                (block_id, cell)
            }
            Some(get_shard_account_cell_request::Criteria::BlockId(block_id)) => {
                let block_id = extend_block_id(&self.client, block_id).await?;
                let cell = self
                    .client
                    .get_shard_account_cell_on_block(&account_address, block_id.clone())
                    .await?;

                (block_id, cell)
            }
            Some(get_shard_account_cell_request::Criteria::TransactionId(tx_id)) => {
                let state = self
                    .client
                    .get_account_state_by_transaction(&account_address, tx_id.clone().into())
                    .await?;
                let cell = self
                    .client
                    .get_shard_account_cell_on_block(&account_address, state.block_id.clone())
                    .await?;

                (state.block_id, cell)
            }
            Some(get_shard_account_cell_request::Criteria::AtLeastBlockId(block_id)) => {
                let block_id = extend_block_id(&self.client, block_id).await?;
                let state = self
                    .client
                    .get_account_state_at_least_block(&account_address, &block_id)
                    .await?;
                let cell = self
                    .client
                    .get_shard_account_cell_on_block(&account_address, state.block_id.clone())
                    .await?;

                (state.block_id, cell)
            }
        };

        Ok((block_id, cell))
    }
}

#[cfg(test)]
mod integration {
    use crate::account::AccountService;
    use crate::ton::account_service_client::AccountServiceClient;
    use crate::ton::account_service_server::AccountServiceServer;
    use crate::ton::{
        BlockId, GetAccountStateRequest, GetAccountTransactionsRequest, GetShardAccountCellRequest,
        get_account_state_request, get_shard_account_cell_request,
    };
    use futures::StreamExt;
    use testcontainers_ton::LocalLiteServer;
    use tokio::net::TcpListener;
    use tonic::transport::Channel;
    use tonlibjson_client::ton::TonClientBuilder;

    const ACCOUNT_ADDRESS: &str =
        "-1:5555555555555555555555555555555555555555555555555555555555555555";

    #[tokio::test]
    async fn should_get_account_state() {
        let (_server, mut accounts) = setup().await;

        let resp = accounts
            .get_account_state(GetAccountStateRequest {
                account_address: ACCOUNT_ADDRESS.to_string(),
                criteria: None,
            })
            .await
            .unwrap()
            .into_inner();

        assert_eq!(resp.account_address, ACCOUNT_ADDRESS);
        assert_eq!(resp.balance, 10000000000);
        let block_id = resp.block_id.unwrap();
        assert_eq!(block_id.workchain, -1);
        assert_eq!(block_id.shard, -9223372036854775808);
        assert!(block_id.seqno > 0);
        assert_eq!(block_id.root_hash.len(), 44);
        assert_eq!(block_id.file_hash.len(), 44);
        let last_tx = resp.last_transaction_id.unwrap();
        assert_eq!(last_tx.hash.len(), 44);
        assert!(last_tx.lt > 0);
        assert!(matches!(
            resp.account_state,
            Some(crate::ton::get_account_state_response::AccountState::Active(_))
        ));
    }

    #[tokio::test]
    async fn should_get_account_state_on_block() {
        let (_server, mut accounts) = setup().await;
        let state = accounts
            .get_account_state(GetAccountStateRequest {
                account_address: ACCOUNT_ADDRESS.to_string(),
                criteria: None,
            })
            .await
            .unwrap()
            .into_inner();
        let block_id = state.block_id.unwrap();

        let resp = accounts
            .get_account_state(GetAccountStateRequest {
                account_address: ACCOUNT_ADDRESS.to_string(),
                criteria: Some(get_account_state_request::Criteria::BlockId(BlockId {
                    workchain: block_id.workchain,
                    shard: block_id.shard,
                    seqno: block_id.seqno,
                    root_hash: Some(block_id.root_hash),
                    file_hash: Some(block_id.file_hash),
                })),
            })
            .await
            .unwrap()
            .into_inner();

        assert_eq!(resp.balance, 10000000000);
        assert!(resp.block_id.is_some());
        assert!(resp.last_transaction_id.is_some());
        assert!(matches!(
            resp.account_state,
            Some(crate::ton::get_account_state_response::AccountState::Active(_))
        ));
    }

    #[tokio::test]
    async fn should_get_account_state_by_transaction() {
        let (_server, mut accounts) = setup().await;
        let state = accounts
            .get_account_state(GetAccountStateRequest {
                account_address: ACCOUNT_ADDRESS.to_string(),
                criteria: None,
            })
            .await
            .unwrap()
            .into_inner();
        let last_tx = state.last_transaction_id.unwrap();

        let resp = accounts
            .get_account_state(GetAccountStateRequest {
                account_address: ACCOUNT_ADDRESS.to_string(),
                criteria: Some(get_account_state_request::Criteria::TransactionId(
                    crate::ton::PartialTransactionId {
                        hash: last_tx.hash,
                        lt: last_tx.lt,
                    },
                )),
            })
            .await
            .unwrap()
            .into_inner();

        assert_eq!(resp.balance, 10000000000);
        assert!(resp.block_id.is_some());
        assert!(matches!(
            resp.account_state,
            Some(crate::ton::get_account_state_response::AccountState::Active(_))
        ));
    }

    #[tokio::test]
    async fn should_get_shard_account_cell() {
        let (_server, mut accounts) = setup().await;

        let resp = accounts
            .get_shard_account_cell(GetShardAccountCellRequest {
                account_address: ACCOUNT_ADDRESS.to_string(),
                criteria: None,
            })
            .await
            .unwrap()
            .into_inner();

        assert_eq!(resp.account_address, ACCOUNT_ADDRESS);
        let block_id = resp.block_id.unwrap();
        assert_eq!(block_id.workchain, -1);
        assert_eq!(block_id.shard, -9223372036854775808);
        assert!(block_id.seqno > 0);
        assert_eq!(block_id.root_hash.len(), 44);
        assert_eq!(block_id.file_hash.len(), 44);
        assert!(!resp.cell.unwrap().bytes.is_empty());
    }

    #[tokio::test]
    async fn should_get_shard_account_cell_on_block() {
        let (_server, mut accounts) = setup().await;
        let cell_resp = accounts
            .get_shard_account_cell(GetShardAccountCellRequest {
                account_address: ACCOUNT_ADDRESS.to_string(),
                criteria: None,
            })
            .await
            .unwrap()
            .into_inner();
        let block_id = cell_resp.block_id.unwrap();

        let resp = accounts
            .get_shard_account_cell(GetShardAccountCellRequest {
                account_address: ACCOUNT_ADDRESS.to_string(),
                criteria: Some(get_shard_account_cell_request::Criteria::BlockId(BlockId {
                    workchain: block_id.workchain,
                    shard: block_id.shard,
                    seqno: block_id.seqno,
                    root_hash: Some(block_id.root_hash),
                    file_hash: Some(block_id.file_hash),
                })),
            })
            .await
            .unwrap()
            .into_inner();

        assert!(resp.cell.is_some());
        assert!(!resp.cell.unwrap().bytes.is_empty());
    }

    #[tokio::test]
    async fn should_get_account_transactions() {
        let (_server, mut accounts) = setup().await;

        let stream = accounts
            .get_account_transactions(GetAccountTransactionsRequest {
                account_address: ACCOUNT_ADDRESS.to_string(),
                order: crate::ton::get_account_transactions_request::Order::FromNewToOld as i32,
                from: None,
                to: None,
            })
            .await
            .unwrap()
            .into_inner();
        let txs: Vec<_> = stream.take(5).collect().await;

        assert!(!txs.is_empty());
        for tx in &txs {
            let tx = tx.as_ref().unwrap();
            let id = tx.id.as_ref().unwrap();
            assert_eq!(id.account_address, ACCOUNT_ADDRESS);
            assert_eq!(id.hash.len(), 44);
            assert!(id.lt > 0);
            assert!(tx.utime > 0);
            assert_eq!(tx.fee, 0);
            assert!(!tx.data.is_empty());
        }
    }

    async fn setup() -> (LocalLiteServer, AccountServiceClient<Channel>) {
        let server = LocalLiteServer::new().await.unwrap();
        let mut client = TonClientBuilder::from_config(server.config())
            .build()
            .unwrap();
        client.ready().await.unwrap();

        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();

        tokio::spawn(async move {
            tonic::transport::Server::builder()
                .add_service(AccountServiceServer::new(AccountService::new(client)))
                .serve_with_incoming(tokio_stream::wrappers::TcpListenerStream::new(listener))
                .await
                .unwrap();
        });

        let channel = Channel::from_shared(format!("http://{}", addr))
            .unwrap()
            .connect()
            .await
            .unwrap();

        (server, AccountServiceClient::new(channel))
    }
}
