use futures::stream::BoxStream;
use futures::StreamExt;
use tonic::{async_trait, Request, Response, Status};
use derive_new::new;
use tonlibjson_client::ton::TonClient;
use crate::ton::block_server::Block;
use crate::ton::{BlockIdExt, SubscribeLastBlockRequest};

#[derive(new)]
pub struct BlockService {
    client: TonClient
}

#[async_trait]
impl Block for BlockService {
    type SubscribeLastBlockStream = BoxStream<'static, Result<BlockIdExt, Status>>;

    async fn subscribe_last_block(&self, _: Request<SubscribeLastBlockRequest>) -> Result<Response<Self::SubscribeLastBlockStream>, Status> {
        Ok(Response::new(futures::stream::empty().boxed()))
    }
}
