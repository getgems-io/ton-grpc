pub mod ext;
pub mod types;

pub use ext::*;
pub use types::*;

use async_trait::async_trait;
use futures::stream::BoxStream;
use std::ops::Bound;

#[async_trait]
pub trait TonClient: Clone + Send + Sync + 'static {
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

    async fn get_shards_by_block_id(&self, block_id: BlockIdExt)
    -> anyhow::Result<Vec<BlockIdExt>>;

    async fn get_block_header(&self, id: BlockIdExt) -> anyhow::Result<BlockHeader>;

    async fn get_account_state(&self, address: &str) -> anyhow::Result<AccountState>;

    async fn get_account_state_on_block(
        &self,
        address: &str,
        block_id: BlockIdExt,
    ) -> anyhow::Result<AccountState>;

    async fn get_account_state_at_least_block(
        &self,
        address: &str,
        block_id: &BlockIdExt,
    ) -> anyhow::Result<AccountState>;

    async fn get_account_state_by_transaction(
        &self,
        address: &str,
        tx: TransactionId,
    ) -> anyhow::Result<AccountState>;

    async fn get_transactions(
        &self,
        address: &str,
        from: &TransactionId,
    ) -> anyhow::Result<Transactions>;

    async fn get_shard_account_cell(&self, address: &str) -> anyhow::Result<Cell>;

    async fn get_shard_account_cell_on_block(
        &self,
        address: &str,
        block: BlockIdExt,
    ) -> anyhow::Result<Cell>;

    async fn get_shard_account_cell_at_least_block(
        &self,
        address: &str,
        block_id: &BlockIdExt,
    ) -> anyhow::Result<Cell>;

    async fn get_shard_account_cell_by_transaction(
        &self,
        address: &str,
        tx: TransactionId,
    ) -> anyhow::Result<Cell>;

    async fn run_get_method(
        &self,
        address: &str,
        method: &str,
        stack: Vec<StackEntry>,
    ) -> anyhow::Result<SmcRunResult>;

    async fn send_message(&self, message: &str) -> anyhow::Result<()>;

    async fn send_message_returning_hash(&self, message: &str) -> anyhow::Result<String>;

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

    fn get_accounts_in_block_stream(
        &self,
        block: &BlockIdExt,
    ) -> BoxStream<'static, anyhow::Result<String>>;

    async fn get_account_tx_range_unordered(
        &self,
        address: &str,
        range: (Bound<TransactionId>, Bound<TransactionId>),
    ) -> anyhow::Result<BoxStream<'static, anyhow::Result<Transaction>>>;
}
