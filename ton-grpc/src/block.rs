use anyhow::Context;
use futures::stream::BoxStream;
use futures::{StreamExt, TryStreamExt};
use tonic::{async_trait, Request, Response, Status};
use derive_new::new;
use tonlibjson_client::ton::TonClient;
use crate::helpers::extend_block_id;
use crate::ton::block_service_server::BlockService as BaseBlockService;
use crate::ton::{AccountAddress, BlockId, BlockIdExt, GetTransactionIdsRequest, GetLastBlockRequest, GetShardsResponse, TransactionId};
use crate::ton::get_transaction_ids_request::Order;

#[derive(new)]
pub struct BlockService {
    client: TonClient
}

#[async_trait]
impl BaseBlockService for BlockService {
    #[tracing::instrument(skip_all, err)]
    async fn get_last_block(&self, _request: Request<GetLastBlockRequest>) -> Result<Response<BlockIdExt>, Status> {
        let block = self.client.get_masterchain_info().await
            .map_err(|e: anyhow::Error| Status::internal(e.to_string()))?.last;

        Ok(Response::new(block.into()))
    }

    #[tracing::instrument(skip_all, err)]
    async fn get_block(&self, request: Request<BlockId>) -> Result<Response<BlockIdExt>, Status> {
        let block_id = extend_block_id(&self.client, &request.into_inner()).await
            .map_err(|e: anyhow::Error| Status::internal(e.to_string()))?;

        Ok(Response::new(block_id.into()))
    }

    #[tracing::instrument(skip_all, err)]
    async fn get_shards(&self, request: Request<BlockId>) -> Result<Response<GetShardsResponse>, Status> {
        let block_id = extend_block_id(&self.client, &request.into_inner()).await
            .map_err(|e: anyhow::Error| Status::internal(e.to_string()))?;

        let shards = self.client.get_shards_by_block_id(block_id).await
            .map_err(|e: anyhow::Error| Status::internal(e.to_string()))?;

        Ok(Response::new(GetShardsResponse {
            shards: shards.into_iter().map(|i| i.into()).collect()
        }))
    }

    type GetTransactionIdsStream = BoxStream<'static, Result<TransactionId, Status>>;

    #[tracing::instrument(skip_all, err)]
    async fn get_transaction_ids(&self, request: Request<GetTransactionIdsRequest>) -> Result<Response<Self::GetTransactionIdsStream>, Status> {
        let msg = request.into_inner();

        let order = msg.order();
        let block_id = msg.block_id.context("block id is required")
            .map_err(|e| Status::internal(e.to_string()))?;

        let chain_id = block_id.workchain;
        let block_id = extend_block_id(&self.client, &block_id).await
            .map_err(|e| Status::internal(e.to_string()))?;

        let stream = match order {
            Order::Unordered => self.client.get_block_tx_stream_unordered(&block_id).boxed(),
            Order::Asc => self.client.get_block_tx_id_stream(&block_id, false).boxed(),
            Order::Desc => self.client.get_block_tx_id_stream(&block_id, true).boxed(),
        };

        let stream = stream
            .map_ok(move |t| { (chain_id, t).into() })
            .map_err(|e| Status::internal(e.to_string()))
            .boxed();

        Ok(Response::new(stream))
    }

    type GetAccountAddressesStream = BoxStream<'static, Result<AccountAddress, Status>>;

    #[tracing::instrument(skip_all, err)]
    async fn get_account_addresses(&self, request: Request<BlockId>) -> Result<Response<Self::GetAccountAddressesStream>, Status> {
        let msg = request.into_inner();
        let block_id = extend_block_id(&self.client, &msg).await
            .map_err(|e: anyhow::Error| Status::internal(e.to_string()))?;

        let stream = self.client.get_accounts_in_block_stream(&block_id)
            .map_ok(|a| AccountAddress { address: a.to_string() })
            .map_err(|e| Status::internal(e.to_string()))
            .boxed();

        Ok(Response::new(stream))
    }
}
