use crate::RequestHandler;
use crate::algo::binary_search::BinarySearch;
use ton_address::SmartContractAddress;
use ton_tower::request::{GetAccountStateOnBlock, LookUpBlockBySeqno};
use ton_tower::response::{AccountState, TransactionId};
use tower::ServiceExt;

pub struct AccountTxAvailability<'a, S> {
    client: &'a mut S,
    address: &'a SmartContractAddress,
    workchain: i32,
    shard: i64,
}

impl<'a, S> AccountTxAvailability<'a, S> {
    pub fn new(
        client: &'a mut S,
        address: &'a SmartContractAddress,
        workchain: i32,
        shard: i64,
    ) -> Self {
        Self {
            client,
            address,
            workchain,
            shard,
        }
    }
}

impl<S> BinarySearch for AccountTxAvailability<'_, S>
where
    S: RequestHandler<LookUpBlockBySeqno> + RequestHandler<GetAccountStateOnBlock> + 'static,
{
    type Item = TransactionId;

    async fn probe(&mut self, point: i32) -> anyhow::Result<Self::Item> {
        let block_id = self
            .client
            .oneshot(LookUpBlockBySeqno {
                chain: self.workchain,
                shard: self.shard,
                seqno: point,
            })
            .await?;

        let state: AccountState = self
            .client
            .oneshot(GetAccountStateOnBlock {
                address: self.address.clone(),
                block_id,
            })
            .await?;

        state
            .last_transaction_id
            .ok_or_else(|| anyhow::anyhow!("tx not found"))
    }
}
