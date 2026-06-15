use crate::RequestHandler;
use crate::algo::binary_search::BinarySearch;
use futures::TryStreamExt;
use std::iter::once;
use ton_tower::request::{GetBlockHeader, GetShards, LookUpBlockBySeqno};
use ton_tower::response::BlockHeader;
use tower::ServiceExt;

pub struct BlockAvailability<'a, S> {
    client: &'a mut S,
    workchain: i32,
    shard: i64,
}

impl<'a, S> BlockAvailability<'a, S> {
    pub fn new(client: &'a mut S, workchain: i32, shard: i64) -> Self {
        Self {
            client,
            workchain,
            shard,
        }
    }
}

impl<S> BinarySearch for BlockAvailability<'_, S>
where
    S: RequestHandler<GetShards>
        + RequestHandler<LookUpBlockBySeqno>
        + RequestHandler<GetBlockHeader>
        + Send
        + 'static,
{
    type Item = Vec<BlockHeader>;

    async fn probe(&mut self, point: i32) -> anyhow::Result<Self::Item> {
        let block_id = self
            .client
            .oneshot(LookUpBlockBySeqno {
                chain: self.workchain,
                shard: self.shard,
                seqno: point,
            })
            .await?;

        let shards = self
            .client
            .oneshot(GetShards {
                block_id: block_id.clone(),
            })
            .await?;

        let requests = once(block_id).chain(shards).map(|id| GetBlockHeader { id });

        self.client
            .call_all(futures::stream::iter(requests))
            .try_collect()
            .await
    }
}
