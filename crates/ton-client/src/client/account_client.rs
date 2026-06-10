use crate::client::Client;
use crate::pool::Forward;
use crate::route::{BlockCriteria, Route};
use crate::{ForwardHandler, RequestHandler};
use ton_address::SmartContractAddress;
use ton_tower::request::{
    GetAccountState, GetAccountStateByTransaction, GetAccountStateOnBlock, GetAccountTransactions,
    GetShardAccountCell, GetShardAccountCellByTransaction, GetShardAccountCellOnBlock,
};
use ton_tower::response::{AccountState, BlockIdExt, Cell, TransactionId, Transactions};
use tower::ServiceExt;

impl<S> Client<S>
where
    S: RequestHandler<GetAccountState>,
{
    pub async fn get_account_state(
        &mut self,
        address: &SmartContractAddress,
    ) -> anyhow::Result<AccountState> {
        let address = address.clone();
        self.oneshot(GetAccountState { address }).await
    }
}

impl<S> Client<S>
where
    S: RequestHandler<GetAccountStateOnBlock>,
{
    pub async fn get_account_state_on_block(
        &mut self,
        address: &SmartContractAddress,
        block_id: BlockIdExt,
    ) -> anyhow::Result<AccountState> {
        let address = address.clone();
        self.oneshot(GetAccountStateOnBlock { address, block_id })
            .await
    }
}

impl<S> Client<S>
where
    S: ForwardHandler<GetAccountState>,
{
    pub async fn get_account_state_at_least_block(
        &mut self,
        address: &SmartContractAddress,
        block_id: &BlockIdExt,
    ) -> anyhow::Result<AccountState> {
        let address = address.clone();
        let route = Route::Block {
            chain: block_id.workchain,
            criteria: BlockCriteria::Seqno {
                shard: block_id.shard,
                seqno: block_id.seqno,
            },
        };
        self.oneshot(Forward::new(route, GetAccountState { address }))
            .await
    }
}

impl<S> Client<S>
where
    S: RequestHandler<GetAccountStateByTransaction>,
{
    pub async fn get_account_state_by_transaction(
        &mut self,
        address: &SmartContractAddress,
        tx: TransactionId,
    ) -> anyhow::Result<AccountState> {
        let address = address.clone();
        self.oneshot(GetAccountStateByTransaction {
            address,
            transaction_id: tx,
        })
        .await
    }
}

impl<S> Client<S>
where
    S: RequestHandler<GetAccountTransactions>,
{
    pub async fn get_transactions(
        &mut self,
        address: &SmartContractAddress,
        from: &TransactionId,
    ) -> anyhow::Result<Transactions> {
        let address = address.clone();
        let from = from.clone();

        self.oneshot(GetAccountTransactions { address, from }).await
    }
}

impl<S> Client<S>
where
    S: RequestHandler<GetShardAccountCell>,
{
    pub async fn get_shard_account_cell(
        &mut self,
        address: &SmartContractAddress,
    ) -> anyhow::Result<Cell> {
        let address = address.clone();
        self.oneshot(GetShardAccountCell { address }).await
    }
}

impl<S> Client<S>
where
    S: RequestHandler<GetShardAccountCellOnBlock>,
{
    pub async fn get_shard_account_cell_on_block(
        &mut self,
        address: &SmartContractAddress,
        block_id: BlockIdExt,
    ) -> anyhow::Result<Cell> {
        let address = address.clone();
        self.oneshot(GetShardAccountCellOnBlock { address, block_id })
            .await
    }
}

impl<S> Client<S>
where
    S: ForwardHandler<GetShardAccountCell>,
{
    pub async fn get_shard_account_cell_at_least_block(
        &mut self,
        address: &SmartContractAddress,
        block_id: &BlockIdExt,
    ) -> anyhow::Result<Cell> {
        let address = address.clone();
        let route = Route::Block {
            chain: block_id.workchain,
            criteria: BlockCriteria::Seqno {
                shard: block_id.shard,
                seqno: block_id.seqno,
            },
        };
        self.oneshot(Forward::new(route, GetShardAccountCell { address }))
            .await
    }
}

impl<S> Client<S>
where
    S: RequestHandler<GetShardAccountCellByTransaction>,
{
    pub async fn get_shard_account_cell_by_transaction(
        &mut self,
        address: &SmartContractAddress,
        tx: TransactionId,
    ) -> anyhow::Result<Cell> {
        let address = address.clone();
        self.oneshot(GetShardAccountCellByTransaction {
            address,
            transaction_id: tx,
        })
        .await
    }
}
