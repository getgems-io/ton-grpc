use futures::stream::BoxStream;
use futures::{StreamExt};
use tonic::{async_trait, Request, Response, Status};
use derive_new::new;
use tonlibjson_client::ton::TonClient;
use crate::helpers::extend_block_id;
use crate::ton::block_server::Block;
use crate::ton::{BlockId, BlockIdExt, GetLastBlockRequest, GetShardsResponse, SubscribeLastBlockEvent, SubscribeLastBlockRequest};

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
}
