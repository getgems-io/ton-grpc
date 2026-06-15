use crate::RequestHandler;
use crate::algo::binary_search::{AccountTxAvailability, BinarySearch};
use crate::client::Client;
use anyhow::anyhow;
use async_stream::try_stream;
use futures::stream::BoxStream;
use futures::{Stream, StreamExt, TryStreamExt, stream_select};
use itertools::Itertools;
use std::cmp::min;
use std::collections::HashMap;
use std::ops::{Bound, RangeBounds};
use ton_address::SmartContractAddress;
use ton_tower::request::{
    GetAccountState, GetAccountStateByTransaction, GetAccountStateOnBlock, GetAccountTransactions,
    GetMasterchainInfo, GetShards, GetTransactionIds, GetTransactions, LookUpBlockBySeqno,
};
use ton_tower::response::{
    AccountState, BlockIdExt, Shards, ShortTxId, Transaction, TransactionId,
};

impl<S> Client<S> {
    pub async fn get_shards(&mut self, master_seqno: i32) -> anyhow::Result<Shards>
    where
        S: RequestHandler<LookUpBlockBySeqno> + RequestHandler<GetShards>,
    {
        let block = self
            .look_up_block_by_seqno(-1, i64::MIN, master_seqno)
            .await?;
        let shards = self.get_shards_by_block_id(block).await?;
        Ok(Shards { shards })
    }
}

impl<S> Client<S>
where
    S: Clone + Send + Sync + 'static,
{
    pub fn get_block_tx_id_stream(
        &self,
        block: &BlockIdExt,
        reverse: bool,
    ) -> impl Stream<Item = anyhow::Result<ShortTxId>> + Send + 'static
    where
        S: RequestHandler<GetTransactionIds>,
    {
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

                let State {
                    after,
                    incomplete: _,
                    block,
                    mut client,
                    exp,
                } = state;

                let txs = client
                    .blocks_get_transactions(&block, after, reverse, 2_i32.pow(exp))
                    .await?;

                let after = txs.transactions.last().cloned();

                anyhow::Ok(Some((
                    futures::stream::iter(txs.transactions.into_iter().map(anyhow::Ok)),
                    State {
                        after,
                        incomplete: txs.incomplete,
                        block,
                        client,
                        exp: min(8, exp + 1),
                    },
                )))
            },
        )
        .try_flatten()
    }

    pub fn get_block_tx_stream(
        &self,
        block: &BlockIdExt,
        reverse: bool,
    ) -> impl Stream<Item = anyhow::Result<Transaction>> + Send + 'static
    where
        S: RequestHandler<GetTransactions>,
    {
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

                let State {
                    after,
                    incomplete: _,
                    block,
                    mut client,
                    exp,
                } = state;

                let txs = client
                    .blocks_get_transactions_ext(&block, after, reverse, 2_i32.pow(exp))
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
                        block,
                        client,
                        exp: min(8, exp + 1),
                    },
                )))
            },
        )
        .try_flatten()
    }

    pub fn get_block_tx_stream_unordered(
        &self,
        block: &BlockIdExt,
    ) -> impl Stream<Item = anyhow::Result<ShortTxId>> + Send + 'static
    where
        S: RequestHandler<GetTransactionIds>,
    {
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

    pub fn get_accounts_in_block_stream(
        &self,
        block: &BlockIdExt,
    ) -> impl Stream<Item = anyhow::Result<SmartContractAddress>> + Send + 'static
    where
        S: RequestHandler<GetTransactionIds>,
    {
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

    pub fn get_account_tx_stream_from(
        &self,
        address: &SmartContractAddress,
        last_tx: Option<TransactionId>,
    ) -> impl Stream<Item = anyhow::Result<Transaction>> + Send + 'static
    where
        S: RequestHandler<GetAccountState> + RequestHandler<GetAccountTransactions>,
    {
        struct State<C> {
            address: SmartContractAddress,
            next_id: Option<TransactionId>,
            client: C,
            next: bool,
        }

        futures::stream::try_unfold(
            State {
                address: address.to_owned(),
                next_id: last_tx,
                client: self.clone(),
                next: true,
            },
            move |state| async move {
                if !state.next {
                    return anyhow::Ok(None);
                }

                let State {
                    address,
                    next_id,
                    mut client,
                    next: _,
                } = state;

                let next_id = if let Some(id) = next_id {
                    id
                } else {
                    let account_state = client.get_account_state(&address).await?;
                    let Some(tx_id) = account_state.last_transaction_id else {
                        return anyhow::Ok(None);
                    };

                    tx_id
                };

                let txs = client.get_transactions(&address, &next_id).await?;

                let next = txs.previous_transaction_id.is_some();
                anyhow::Ok(Some((
                    futures::stream::iter(txs.transactions.into_iter().map(anyhow::Ok)),
                    State {
                        address,
                        next_id: txs.previous_transaction_id,
                        client,
                        next,
                    },
                )))
            },
        )
        .try_flatten()
    }

    pub fn get_account_tx_stream(
        &self,
        address: &SmartContractAddress,
    ) -> impl Stream<Item = anyhow::Result<Transaction>> + Send + 'static
    where
        S: RequestHandler<GetAccountState> + RequestHandler<GetAccountTransactions>,
    {
        self.get_account_tx_stream_from(address, None)
    }

    pub async fn get_account_tx_range_unordered(
        &mut self,
        address: &SmartContractAddress,
        range: (Bound<TransactionId>, Bound<TransactionId>),
    ) -> anyhow::Result<BoxStream<'static, anyhow::Result<Transaction>>>
    where
        S: RequestHandler<GetMasterchainInfo>
            + RequestHandler<LookUpBlockBySeqno>
            + RequestHandler<GetAccountState>
            + RequestHandler<GetAccountStateOnBlock>
            + RequestHandler<GetAccountStateByTransaction>
            + RequestHandler<GetAccountTransactions>,
    {
        let address = address.to_owned();

        let mut client_first = self.clone();
        let address_first = address.clone();
        let (last_tx, first_tx) = futures::try_join!(
            async {
                let last_tx = match range.start_bound().cloned() {
                    Bound::Included(tx) | Bound::Excluded(tx) => tx,
                    Bound::Unbounded => {
                        let state: AccountState = self.get_account_state(&address).await?;
                        state
                            .last_transaction_id
                            .ok_or_else(|| anyhow!("invalid last tx"))?
                    }
                };
                anyhow::Ok(last_tx)
            },
            async {
                let first_tx = match range.end_bound().cloned() {
                    Bound::Included(tx) | Bound::Excluded(tx) => tx,
                    Bound::Unbounded => {
                        AccountTxAvailability::new(&mut client_first, &address_first)
                            .find()
                            .await?
                    }
                };
                anyhow::Ok(first_tx)
            }
        )?;

        let last_block = self
            .get_account_state_by_transaction(&address, last_tx.clone())
            .await?
            .block_id;
        let first_block = self
            .get_account_state_by_transaction(&address, first_tx.clone())
            .await?
            .block_id;

        let chunks = min(256, (last_block.seqno - first_block.seqno) / 28800);
        let step = (last_block.seqno - first_block.seqno) / chunks;

        let workchain = first_block.workchain;
        let shard = first_block.shard;
        let seqno = first_block.seqno;

        let mid: Vec<anyhow::Result<TransactionId>> = futures::stream::iter(1..chunks)
            .map(|i| {
                let mut client = self.clone();
                let address = address.clone();
                async move {
                    let block = client
                        .look_up_block_by_seqno(workchain, shard, seqno + step * i)
                        .await?;
                    let state = client.get_account_state_on_block(&address, block).await?;

                    anyhow::Ok(
                        state
                            .last_transaction_id
                            .ok_or(anyhow!("invalid last tx"))?,
                    )
                }
            })
            .buffered(32)
            .collect()
            .await;

        let mut mid = mid
            .into_iter()
            .collect::<anyhow::Result<Vec<TransactionId>>>()?;

        let mut txs = vec![first_tx.clone()];
        txs.append(&mut mid);
        txs.push(last_tx.clone());
        txs.dedup();

        let streams = txs
            .array_windows::<2>()
            .map(|[left, right]| {
                let left_bound = if left == &first_tx {
                    range.end_bound().cloned()
                } else {
                    Bound::Included(left.clone())
                };
                let right_bound = if right == &last_tx {
                    range.start_bound().cloned()
                } else {
                    Bound::Excluded(right.clone())
                };

                self.get_account_tx_range(&address, (right_bound, left_bound))
                    .boxed()
            })
            .collect_vec();

        Ok(
            Box::pin(futures::stream::iter(streams).flatten_unordered(32))
                as BoxStream<'static, anyhow::Result<Transaction>>,
        )
    }

    pub fn get_account_tx_range(
        &self,
        address: &SmartContractAddress,
        range: (Bound<TransactionId>, Bound<TransactionId>),
    ) -> impl Stream<Item = anyhow::Result<Transaction>> + Send + 'static
    where
        S: RequestHandler<GetAccountState> + RequestHandler<GetAccountTransactions>,
    {
        let last_tx: Option<TransactionId> = match range.start_bound() {
            Bound::Included(tx) | Bound::Excluded(tx) => Some(tx.to_owned()),
            Bound::Unbounded => None,
        };
        let stream = self.get_account_tx_stream_from(address, last_tx);

        let exclude: Option<TransactionId> =
            if let Bound::Excluded(tx) = range.start_bound().cloned() {
                Some(tx)
            } else {
                None
            };
        let stream = stream.try_skip_while(move |sx| {
            std::future::ready(if let Some(tx) = exclude.as_ref() {
                Ok(tx == &sx.transaction_id)
            } else {
                Ok(false)
            })
        });

        let end: Bound<TransactionId> = range.end_bound().cloned();
        try_stream! {
            futures::pin_mut!(stream);
            while let Some(x) = stream.try_next().await? {
                match end.as_ref() {
                    Bound::Unbounded => { yield x; },
                    Bound::Included(tx) => {
                        let cond = tx == &x.transaction_id;
                        yield x;
                        if cond { break; }
                    },
                    Bound::Excluded(tx) => {
                        if tx == &x.transaction_id { break; }
                        yield x;
                    }
                }
            }
        }
    }
}
