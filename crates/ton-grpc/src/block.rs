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
    use crate::ton::GetLastBlockRequest;
    use crate::ton::block_service_client::BlockServiceClient;
    use crate::ton::block_service_server::BlockServiceServer;
    use testcontainers_ton::LocalLiteServer;
    use tokio::net::TcpListener;
    use tonic::transport::Channel;
    use tonlibjson_client::ton::TonClientBuilder;

    #[tokio::test]
    async fn should_get_last_block() {
        let mut client = setup().await;

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

    async fn setup() -> BlockServiceClient<Channel> {
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

        BlockServiceClient::new(channel)
    }
}
