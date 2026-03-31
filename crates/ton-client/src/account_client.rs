use async_trait::async_trait;

use crate::{AccountState, BlockIdExt, Cell, TransactionId, Transactions};

#[async_trait]
pub trait AccountClient: Clone + Send + Sync + 'static {
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
}
