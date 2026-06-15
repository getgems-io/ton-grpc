use crate::RequestHandler;
use crate::algo::binary_search::BinarySearch;
use futures::TryFutureExt;
use ton_address::SmartContractAddress;
use ton_tower::request::{GetAccountStateOnBlock, GetMasterchainInfo, LookUpBlockBySeqno};
use ton_tower::response::{AccountState, BlockId, TransactionId};
use tower::ServiceExt;

pub struct AccountTxAvailability<'a, S> {
    client: &'a mut S,
    address: &'a SmartContractAddress,
}

impl<'a, S> AccountTxAvailability<'a, S> {
    pub fn new(client: &'a mut S, address: &'a SmartContractAddress) -> Self {
        Self { client, address }
    }
}

impl<S> BinarySearch for AccountTxAvailability<'_, S>
where
    S: RequestHandler<GetMasterchainInfo>
        + RequestHandler<LookUpBlockBySeqno>
        + RequestHandler<GetAccountStateOnBlock>
        + 'static,
{
    type Item = TransactionId;

    async fn probe(&mut self, point: BlockId) -> anyhow::Result<Self::Item> {
        let block_id = self
            .client
            .oneshot(LookUpBlockBySeqno {
                chain: point.workchain,
                shard: point.shard,
                seqno: point.seqno,
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

    async fn upper_bound(&mut self) -> anyhow::Result<BlockId> {
        self.client
            .oneshot(GetMasterchainInfo::default())
            .map_ok(|r| r.last.into())
            .await
    }
}
