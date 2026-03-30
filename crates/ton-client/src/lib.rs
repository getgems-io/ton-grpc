pub mod types;

pub use types::*;

use async_trait::async_trait;

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

    async fn get_block_header(&self, id: BlockIdExt) -> anyhow::Result<BlockHeader>;

    async fn get_account_state(&self, address: &str) -> anyhow::Result<AccountState>;

    async fn get_account_state_on_block(
        &self,
        address: &str,
        block_id: BlockIdExt,
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
}
