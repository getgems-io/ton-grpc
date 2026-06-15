use crate::RequestHandler;
use crate::algo::binary_search::BinarySearch;
use futures::{TryFutureExt, TryStreamExt};
use std::iter::once;
use ton_tower::request::{GetBlockHeader, GetMasterchainInfo, GetShards, LookUpBlockBySeqno};
use ton_tower::response::{BlockHeader, BlockId};
use tower::ServiceExt;

pub struct BlockAvailability<'a, S> {
    client: &'a mut S,
}

impl<'a, S> BlockAvailability<'a, S> {
    pub fn new(client: &'a mut S) -> Self {
        Self { client }
    }
}

impl<S> BinarySearch for BlockAvailability<'_, S>
where
    S: RequestHandler<GetMasterchainInfo>
        + RequestHandler<GetShards>
        + RequestHandler<LookUpBlockBySeqno>
        + RequestHandler<GetBlockHeader>
        + Send
        + 'static,
{
    type Item = Vec<BlockHeader>;

    async fn probe(&mut self, point: BlockId) -> anyhow::Result<Self::Item> {
        let block_id = self
            .client
            .oneshot(LookUpBlockBySeqno {
                chain: point.workchain,
                shard: point.shard,
                seqno: point.seqno,
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

    async fn upper_bound(&mut self) -> anyhow::Result<BlockId> {
        self.client
            .oneshot(GetMasterchainInfo::default())
            .map_ok(|r| r.last.into())
            .await
    }
}
