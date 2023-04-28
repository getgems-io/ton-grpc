use anyhow::Context;
use futures::stream::BoxStream;
use futures::{StreamExt, TryStreamExt};
use tonic::{async_trait, Request, Response, Status};
use derive_new::new;
use tonlibjson_client::ton::TonClient;
use crate::helpers::extend_block_id;
use crate::ton::block_server::Block;
use crate::ton::{BlockId, BlockIdExt, GetBlockTransactionIdsRequest, GetLastBlockRequest, GetShardsResponse, SubscribeLastBlockEvent, SubscribeLastBlockRequest, TransactionId};

#[derive(new)]
pub struct BlockService {
    client: TonClient
}

#[async_trait]
impl Block for BlockService {
    type SubscribeLastBlockStream = BoxStream<'static, Result<SubscribeLastBlockEvent, Status>>;

    async fn subscribe_last_block(&self, _: Request<SubscribeLastBlockRequest>) -> Result<Response<Self::SubscribeLastBlockStream>, Status> {
        let stream = self.client.last_block_stream()
            .map(|(m, w)| Ok(SubscribeLastBlockEvent {
                masterchain: Some(m.id.into()),
                workchain: Some(w.id.into()),
            }))
            .boxed();

        Ok(Response::new(stream))
    }

    async fn get_last_block(&self, _request: Request<GetLastBlockRequest>) -> Result<Response<BlockIdExt>, Status> {
        let block = self.client.get_masterchain_info().await
            .map_err(|e: anyhow::Error| Status::internal(e.to_string()))?.last;

        Ok(Response::new(block.into()))
    }

    async fn get_block(&self, request: Request<BlockId>) -> Result<Response<BlockIdExt>, Status> {
        let block_id = extend_block_id(&self.client, &request.into_inner()).await
            .map_err(|e: anyhow::Error| Status::internal(e.to_string()))?;

        Ok(Response::new(block_id.into()))
    }

    async fn get_shards(&self, request: Request<BlockId>) -> Result<Response<GetShardsResponse>, Status> {
        let block_id = extend_block_id(&self.client, &request.into_inner()).await
            .map_err(|e: anyhow::Error| Status::internal(e.to_string()))?;

        let shards = self.client.get_shards_by_block_id(block_id).await
            .map_err(|e: anyhow::Error| Status::internal(e.to_string()))?;

        Ok(Response::new(GetShardsResponse {
            shards: shards.into_iter().map(|i| i.into()).collect()
        }))
    }

    type GetBlockTransactionIdsStream = BoxStream<'static, Result<TransactionId, Status>>;

    async fn get_block_transaction_ids(&self, request: Request<GetBlockTransactionIdsRequest>) -> Result<Response<Self::GetBlockTransactionIdsStream>, Status> {
        let msg = request.into_inner();

        let block_id = msg.block_id.context("block id is required")
            .map_err(|e| Status::internal(e.to_string()))?;

        let chain_id = block_id.workchain;

        let block_id = extend_block_id(&self.client, &block_id).await
            .map_err(|e| Status::internal(e.to_string()))?;

        let stream = self.client.get_tx_stream(block_id)
            .and_then(move |t| async move { (chain_id, t).try_into() })
            .map_err(|e| Status::internal(e.to_string()))
            .boxed();

        Ok(Response::new(stream))
    }
}
