#![allow(clippy::blocks_in_conditions)]

use crate::helpers::{extend_block_id, extend_get_block_header};
use crate::ton::block_service_server::BlockService as BaseBlockService;
use crate::ton::get_transaction_ids_request::Order;
use crate::ton::{
    AccountAddress, BlockId, BlockIdExt, BlocksHeader, GetLastBlockRequest, GetShardsResponse,
    GetTransactionIdsRequest, GetTransactionsRequest, Transaction, TransactionId,
};
use anyhow::Context;
use derive_new::new;
use futures::stream::BoxStream;
use futures::{StreamExt, TryStreamExt};
use tonic::{Request, Response, Status, async_trait};
use tonlibjson_client::ton::TonClient;

#[derive(new)]
pub struct BlockService {
    client: TonClient,
}

#[async_trait]
impl BaseBlockService for BlockService {
    #[tracing::instrument(skip_all, err)]
    async fn get_last_block(
        &self,
        _request: Request<GetLastBlockRequest>,
    ) -> Result<Response<BlockIdExt>, Status> {
        let block = self
            .client
            .get_masterchain_info()
            .await
            .map_err(|e: anyhow::Error| Status::internal(e.to_string()))?
            .last;

        Ok(Response::new(block.into()))
    }

    #[tracing::instrument(skip_all, err)]
    async fn get_block(&self, request: Request<BlockId>) -> Result<Response<BlockIdExt>, Status> {
        let block_id = extend_block_id(&self.client, &request.into_inner())
            .await
            .map_err(|e: anyhow::Error| Status::internal(e.to_string()))?;

        Ok(Response::new(block_id.into()))
    }

    #[tracing::instrument(skip_all, err)]
    async fn get_block_header(
        &self,
        request: Request<BlockId>,
    ) -> Result<Response<BlocksHeader>, Status> {
        let block_header = extend_get_block_header(&self.client, &request.into_inner())
            .await
            .map_err(|e: anyhow::Error| Status::internal(e.to_string()))?;

        Ok(Response::new(block_header.into()))
    }

    #[tracing::instrument(skip_all, err)]
    async fn get_shards(
        &self,
        request: Request<BlockId>,
    ) -> Result<Response<GetShardsResponse>, Status> {
        let block_id = extend_block_id(&self.client, &request.into_inner())
            .await
            .map_err(|e: anyhow::Error| Status::internal(e.to_string()))?;

        let shards = self
            .client
            .get_shards_by_block_id(block_id)
            .await
            .map_err(|e: anyhow::Error| Status::internal(e.to_string()))?;

        Ok(Response::new(GetShardsResponse {
            shards: shards.into_iter().map(|i| i.into()).collect(),
        }))
    }

    type GetTransactionIdsStream = BoxStream<'static, Result<TransactionId, Status>>;

    #[tracing::instrument(skip_all, err)]
    async fn get_transaction_ids(
        &self,
        request: Request<GetTransactionIdsRequest>,
    ) -> Result<Response<Self::GetTransactionIdsStream>, Status> {
        let msg = request.into_inner();

        let order = msg.order();
        let block_id = msg
            .block_id
            .context("block id is required")
            .map_err(|e| Status::internal(e.to_string()))?;

        let chain_id = block_id.workchain;
        let block_id = extend_block_id(&self.client, &block_id)
            .await
            .map_err(|e| Status::internal(e.to_string()))?;

        let stream = match order {
            Order::Unordered => self.client.get_block_tx_stream_unordered(&block_id).boxed(),
            Order::Asc => self.client.get_block_tx_id_stream(&block_id, false).boxed(),
            Order::Desc => self.client.get_block_tx_id_stream(&block_id, true).boxed(),
        };

        let stream = stream
            .map_ok(move |t| (chain_id, t).into())
            .map_err(|e| Status::internal(e.to_string()))
            .boxed();

        Ok(Response::new(stream))
    }

    type GetAccountAddressesStream = BoxStream<'static, Result<AccountAddress, Status>>;

    #[tracing::instrument(skip_all, err)]
    async fn get_account_addresses(
        &self,
        request: Request<BlockId>,
    ) -> Result<Response<Self::GetAccountAddressesStream>, Status> {
        let msg = request.into_inner();
        let block_id = extend_block_id(&self.client, &msg)
            .await
            .map_err(|e: anyhow::Error| Status::internal(e.to_string()))?;

        let stream = self
            .client
            .get_accounts_in_block_stream(&block_id)
            .map_ok(|a| AccountAddress {
                address: a.to_string(),
            })
            .map_err(|e| Status::internal(e.to_string()))
            .boxed();

        Ok(Response::new(stream))
    }

    type GetTransactionsStream = BoxStream<'static, Result<Transaction, Status>>;

    async fn get_transactions(
        &self,
        request: Request<GetTransactionsRequest>,
    ) -> Result<Response<Self::GetTransactionsStream>, Status> {
        let msg = request.into_inner();

        // TODO[akostylev0]
        let _order = msg.order();
        let block_id = msg
            .block_id
            .context("block id is required")
            .map_err(|e| Status::internal(e.to_string()))?;

        let chain_id = block_id.workchain;
        let block_id = extend_block_id(&self.client, &block_id)
            .await
            .map_err(|e| Status::internal(e.to_string()))?;

        let stream = self.client.get_block_tx_stream(&block_id, false).boxed();

        let stream = stream
            .map(move |tx| match tx {
                Ok(tx) => (chain_id, tx).try_into(),
                Err(e) => Err(e),
            })
            .map_err(|e| Status::internal(e.to_string()))
            .boxed();

        Ok(Response::new(stream))
    }
}

#[cfg(test)]
mod integration {
    use crate::block::BlockService;
    use crate::ton::block_service_client::BlockServiceClient;
    use crate::ton::block_service_server::BlockServiceServer;
    use crate::ton::{BlockId, GetLastBlockRequest, GetTransactionIdsRequest, GetTransactionsRequest};
    use futures::StreamExt;
    use testcontainers_ton::LocalLiteServer;
    use tokio::net::TcpListener;
    use tonlibjson_client::ton::TonClientBuilder;
    use tonic::transport::Channel;

    #[tokio::test]
    async fn should_get_last_block() {
        let (_server, mut client) = setup().await;

        let resp = client
            .get_last_block(GetLastBlockRequest {})
            .await
            .unwrap()
            .into_inner();

        assert_eq!(resp.workchain, -1);
        assert!(resp.seqno > 0);
        assert_eq!(resp.root_hash.len(), 44);
        assert_eq!(resp.file_hash.len(), 44);
    }

    #[tokio::test]
    async fn should_get_block() {
        let (_server, mut client) = setup().await;
        let last = client
            .get_last_block(GetLastBlockRequest {})
            .await
            .unwrap()
            .into_inner();

        let resp = client
            .get_block(BlockId {
                workchain: last.workchain,
                shard: last.shard,
                seqno: last.seqno,
                root_hash: None,
                file_hash: None,
            })
            .await
            .unwrap()
            .into_inner();

        assert_eq!(resp.workchain, last.workchain);
        assert_eq!(resp.shard, last.shard);
        assert_eq!(resp.seqno, last.seqno);
        assert_eq!(resp.root_hash.len(), 44);
        assert_eq!(resp.file_hash.len(), 44);
    }

    #[tokio::test]
    async fn should_get_block_header() {
        let (_server, mut client) = setup().await;
        let last = client
            .get_last_block(GetLastBlockRequest {})
            .await
            .unwrap()
            .into_inner();
        let seqno = last.seqno - 1;

        let header = client
            .get_block_header(BlockId {
                workchain: last.workchain,
                shard: last.shard,
                seqno,
                root_hash: None,
                file_hash: None,
            })
            .await
            .unwrap()
            .into_inner();

        let id = header.id.unwrap();
        assert_eq!(id.workchain, -1);
        assert_eq!(id.seqno, seqno);
        assert_eq!(id.root_hash.len(), 44);
        assert_eq!(id.file_hash.len(), 44);
        assert!(header.end_lt >= header.start_lt);
        assert!(header.gen_utime > 0);
        assert!(!header.prev_blocks.is_empty());
    }

    #[tokio::test]
    async fn should_get_shards() {
        let (_server, mut client) = setup().await;
        let last = client
            .get_last_block(GetLastBlockRequest {})
            .await
            .unwrap()
            .into_inner();

        let resp = client
            .get_shards(BlockId {
                workchain: last.workchain,
                shard: last.shard,
                seqno: last.seqno,
                root_hash: None,
                file_hash: None,
            })
            .await
            .unwrap()
            .into_inner();

        assert!(!resp.shards.is_empty());
        for shard in &resp.shards {
            assert_eq!(shard.root_hash.len(), 44);
            assert_eq!(shard.file_hash.len(), 44);
        }
    }

    #[tokio::test]
    async fn should_get_transaction_ids() {
        let (_server, mut client) = setup().await;
        let last = client
            .get_last_block(GetLastBlockRequest {})
            .await
            .unwrap()
            .into_inner();

        let stream = client
            .get_transaction_ids(GetTransactionIdsRequest {
                block_id: Some(BlockId {
                    workchain: last.workchain,
                    shard: last.shard,
                    seqno: last.seqno,
                    root_hash: None,
                    file_hash: None,
                }),
                order: 0,
            })
            .await
            .unwrap()
            .into_inner();
        let txs: Vec<_> = stream.collect().await;

        assert!(!txs.is_empty());
        for tx in &txs {
            let tx = tx.as_ref().unwrap();
            assert!(!tx.account_address.is_empty());
            assert!(tx.lt > 0);
            assert!(!tx.hash.is_empty());
        }
    }

    #[tokio::test]
    async fn should_get_account_addresses() {
        let (_server, mut client) = setup().await;
        let last = client
            .get_last_block(GetLastBlockRequest {})
            .await
            .unwrap()
            .into_inner();

        let stream = client
            .get_account_addresses(BlockId {
                workchain: last.workchain,
                shard: last.shard,
                seqno: last.seqno,
                root_hash: None,
                file_hash: None,
            })
            .await
            .unwrap()
            .into_inner();
        let addresses: Vec<_> = stream.collect().await;

        assert!(!addresses.is_empty());
        for addr in &addresses {
            let addr = addr.as_ref().unwrap();
            assert!(!addr.address.is_empty());
        }
    }

    #[tokio::test]
    async fn should_get_transactions() {
        let (_server, mut client) = setup().await;
        let last = client
            .get_last_block(GetLastBlockRequest {})
            .await
            .unwrap()
            .into_inner();

        let stream = client
            .get_transactions(GetTransactionsRequest {
                block_id: Some(BlockId {
                    workchain: last.workchain,
                    shard: last.shard,
                    seqno: last.seqno,
                    root_hash: None,
                    file_hash: None,
                }),
                order: 0,
            })
            .await
            .unwrap()
            .into_inner();
        let txs: Vec<_> = stream.collect().await;

        assert!(!txs.is_empty());
        for tx in &txs {
            let tx = tx.as_ref().unwrap();
            assert!(tx.id.is_some());
            assert!(tx.utime > 0);
            assert!(!tx.data.is_empty());
        }
    }

    async fn setup() -> (LocalLiteServer, BlockServiceClient<Channel>) {
        let server = LocalLiteServer::new().await.unwrap();
        let mut client = TonClientBuilder::from_config(server.config())
            .build()
            .unwrap();
        client.ready().await.unwrap();

        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();

        tokio::spawn(async move {
            tonic::transport::Server::builder()
                .add_service(BlockServiceServer::new(BlockService::new(client)))
                .serve_with_incoming(tokio_stream::wrappers::TcpListenerStream::new(listener))
                .await
                .unwrap();
        });

        let channel = Channel::from_shared(format!("http://{}", addr))
            .unwrap()
            .connect()
            .await
            .unwrap();

        (server, BlockServiceClient::new(channel))
    }
}
