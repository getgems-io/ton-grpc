use crate::{BlockClient, BlockIdExt, Shards, ShortTxId, Transaction};
use async_stream::try_stream;
use futures::stream_select;
use futures::{Stream, StreamExt, TryStreamExt};
use std::cmp::min;
use std::collections::HashMap;
use ton_address::SmartContractAddress;

pub trait BlockClientExt: BlockClient {
    fn get_shards(
        &self,
        master_seqno: i32,
    ) -> impl std::future::Future<Output = anyhow::Result<Shards>> + Send {
        let client = self.clone();
        async move {
            let block = client
                .look_up_block_by_seqno(-1, i64::MIN, master_seqno)
                .await?;

            let shards = client.get_shards_by_block_id(block).await?;

            Ok(Shards { shards })
        }
    }

    fn get_block_tx_id_stream(
        &self,
        block: &BlockIdExt,
        reverse: bool,
    ) -> impl Stream<Item = anyhow::Result<ShortTxId>> + Send + 'static {
        struct State<C> {
            after: Option<ShortTxId>,
            incomplete: bool,
            block: BlockIdExt,
            client: C,
            exp: u32,
        }

        futures::stream::try_unfold(
            State {
                after: None,
                incomplete: true,
                block: block.clone(),
                client: self.clone(),
                exp: 5,
            },
            move |state| async move {
                if !state.incomplete {
                    return anyhow::Ok(None);
                }

                let txs = state
                    .client
                    .blocks_get_transactions(
                        &state.block,
                        state.after,
                        reverse,
                        2_i32.pow(state.exp),
                    )
                    .await?;

                let after = txs.transactions.last().cloned();

                anyhow::Ok(Some((
                    futures::stream::iter(txs.transactions.into_iter().map(anyhow::Ok)),
                    State {
                        after,
                        incomplete: txs.incomplete,
                        block: state.block,
                        client: state.client,
                        exp: min(8, state.exp + 1),
                    },
                )))
            },
        )
        .try_flatten()
    }

    fn get_block_tx_stream(
        &self,
        block: &BlockIdExt,
        reverse: bool,
    ) -> impl Stream<Item = anyhow::Result<Transaction>> + Send + 'static {
        struct State<C> {
            after: Option<ShortTxId>,
            incomplete: bool,
            block: BlockIdExt,
            client: C,
            exp: u32,
        }

        futures::stream::try_unfold(
            State {
                after: None,
                incomplete: true,
                block: block.clone(),
                client: self.clone(),
                exp: 5,
            },
            move |state| async move {
                if !state.incomplete {
                    return anyhow::Ok(None);
                }

                let txs = state
                    .client
                    .blocks_get_transactions_ext(
                        &state.block,
                        state.after,
                        reverse,
                        2_i32.pow(state.exp),
                    )
                    .await?;

                let after = txs.transactions.last().map(|t| ShortTxId {
                    account: t.address.clone(),
                    lt: t.transaction_id.lt,
                    hash: t.transaction_id.hash.clone(),
                });

                anyhow::Ok(Some((
                    futures::stream::iter(txs.transactions.into_iter().map(anyhow::Ok)),
                    State {
                        after,
                        incomplete: txs.incomplete,
                        block: state.block,
                        client: state.client,
                        exp: min(8, state.exp + 1),
                    },
                )))
            },
        )
        .try_flatten()
    }

    fn get_block_tx_stream_unordered(
        &self,
        block: &BlockIdExt,
    ) -> impl Stream<Item = anyhow::Result<ShortTxId>> + Send + 'static {
        let fwd = self.get_block_tx_id_stream(block, false).boxed();
        let rev = self.get_block_tx_id_stream(block, true).boxed();

        let merged = stream_select!(fwd.map(|r| (false, r)), rev.map(|r| (true, r)));

        try_stream! {
            let mut last: HashMap<bool, ShortTxId> = HashMap::with_capacity(2);

            for await (key, tx) in merged {
                let tx: ShortTxId = tx?;
                if let Some(prev_tx) = last.get(&!key)
                    && prev_tx == &tx
                {
                    return;
                }
                last.insert(key, tx.clone());
                yield tx;
            }
        }
    }

    fn get_accounts_in_block_stream(
        &self,
        block: &BlockIdExt,
    ) -> impl Stream<Item = anyhow::Result<SmartContractAddress>> + Send + 'static {
        let fwd = self.get_block_tx_id_stream(block, false).boxed();
        let rev = self.get_block_tx_id_stream(block, true).boxed();

        let merged = stream_select!(fwd.map(|r| (false, r)), rev.map(|r| (true, r)));

        try_stream! {
            let mut last: HashMap<bool, SmartContractAddress> = HashMap::with_capacity(2);

            for await (key, tx) in merged {
                let tx: ShortTxId = tx?;
                if let Some(prev) = last.get(&!key)
                    && prev == &tx.account
                {
                    return;
                }
                if let Some(prev) = last.get(&key)
                    && prev == &tx.account
                {
                    continue;
                }
                last.insert(key, tx.account.clone());
                yield tx.account;
            }
        }
    }
}

impl<T: BlockClient> BlockClientExt for T {}
