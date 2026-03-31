use async_trait::async_trait;

use crate::{
    BlockHeader, BlockIdExt, BlockTransactions, BlockTransactionsExt, MasterchainInfo, Shards,
    ShortTxId,
};

#[async_trait]
pub trait BlockClient: Clone + Send + Sync + 'static {
    async fn get_masterchain_info(&self) -> anyhow::Result<MasterchainInfo>;

    async fn look_up_block_by_seqno(
        &self,
        chain: i32,
        shard: i64,
        seqno: i32,
    ) -> anyhow::Result<BlockIdExt>;

    async fn look_up_block_by_lt(
        &self,
        chain: i32,
        shard: i64,
        lt: i64,
    ) -> anyhow::Result<BlockIdExt>;

    async fn get_shards(&self, master_seqno: i32) -> anyhow::Result<Shards>;

    async fn get_shards_by_block_id(
        &self,
        block_id: BlockIdExt,
    ) -> anyhow::Result<Vec<BlockIdExt>>;

    async fn get_block_header(&self, id: BlockIdExt) -> anyhow::Result<BlockHeader>;

    async fn blocks_get_transactions(
        &self,
        block: &BlockIdExt,
        after: Option<ShortTxId>,
        reverse: bool,
        count: i32,
    ) -> anyhow::Result<BlockTransactions>;

    async fn blocks_get_transactions_ext(
        &self,
        block: &BlockIdExt,
        after: Option<ShortTxId>,
        reverse: bool,
        count: i32,
    ) -> anyhow::Result<BlockTransactionsExt>;
}
