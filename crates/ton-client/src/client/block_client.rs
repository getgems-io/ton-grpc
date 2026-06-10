use crate::{Client, RequestHandler};
use ton_tower::request::{
    GetBlockHeader, GetMasterchainInfo, GetShards, GetTransactionIds, GetTransactions,
    LookUpBlockByLt, LookUpBlockBySeqno, Sync,
};
use ton_tower::response::{
    BlockHeader, BlockIdExt, BlockTransactions, BlockTransactionsExt, MasterchainInfo, ShortTxId,
};
use tower::ServiceExt;

impl<S> Client<S>
where
    S: RequestHandler<GetMasterchainInfo>,
{
    pub async fn get_masterchain_info(&mut self) -> anyhow::Result<MasterchainInfo> {
        self.oneshot(GetMasterchainInfo::default()).await
    }
}

impl<S> Client<S>
where
    S: RequestHandler<Sync>,
{
    pub async fn sync(&mut self) -> anyhow::Result<BlockIdExt> {
        self.oneshot(Sync::default()).await
    }
}

impl<S> Client<S>
where
    S: RequestHandler<LookUpBlockBySeqno>,
{
    pub async fn look_up_block_by_seqno(
        &mut self,
        chain: i32,
        shard: i64,
        seqno: i32,
    ) -> anyhow::Result<BlockIdExt> {
        self.oneshot(LookUpBlockBySeqno {
            chain,
            shard,
            seqno,
        })
        .await
    }
}

impl<S> Client<S>
where
    S: RequestHandler<LookUpBlockByLt>,
{
    pub async fn look_up_block_by_lt(
        &mut self,
        chain: i32,
        shard: i64,
        lt: i64,
    ) -> anyhow::Result<BlockIdExt> {
        self.oneshot(LookUpBlockByLt { chain, shard, lt }).await
    }
}

impl<S> Client<S>
where
    S: RequestHandler<GetShards>,
{
    pub async fn get_shards_by_block_id(
        &mut self,
        block_id: BlockIdExt,
    ) -> anyhow::Result<Vec<BlockIdExt>> {
        self.oneshot(GetShards { block_id }).await
    }
}

impl<S> Client<S>
where
    S: RequestHandler<GetBlockHeader>,
{
    pub async fn get_block_header(&mut self, id: BlockIdExt) -> anyhow::Result<BlockHeader> {
        self.oneshot(GetBlockHeader { id }).await
    }
}

impl<S> Client<S>
where
    S: RequestHandler<GetTransactionIds>,
{
    pub async fn blocks_get_transactions(
        &mut self,
        block: &BlockIdExt,
        after: Option<ShortTxId>,
        reverse: bool,
        count: i32,
    ) -> anyhow::Result<BlockTransactions> {
        let block = block.clone();
        self.oneshot(GetTransactionIds {
            block,
            after,
            reverse,
            count,
        })
        .await
    }
}

impl<S> Client<S>
where
    S: RequestHandler<GetTransactions>,
{
    pub async fn blocks_get_transactions_ext(
        &mut self,
        block: &BlockIdExt,
        after: Option<ShortTxId>,
        reverse: bool,
        count: i32,
    ) -> anyhow::Result<BlockTransactionsExt> {
        let block = block.clone();
        self.oneshot(GetTransactions {
            block,
            after,
            reverse,
            count,
        })
        .await
    }
}
