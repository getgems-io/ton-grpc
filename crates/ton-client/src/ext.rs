use anyhow::anyhow;
use async_stream::try_stream;
use futures::stream::BoxStream;
use futures::stream_select;
use futures::{Stream, StreamExt, TryStreamExt, stream};
use itertools::Itertools;
use std::cmp::min;
use std::collections::HashMap;
use std::ops::{Bound, RangeBounds};
use tracing::{debug, trace};

use crate::{AccountState, BlockIdExt, ShortTxId, TonClient, Transaction, TransactionId};

pub trait TonClientExt: TonClient {
    fn get_account_tx_stream_from(
        &self,
        address: &str,
        last_tx: Option<TransactionId>,
    ) -> impl Stream<Item = anyhow::Result<Transaction>> + Send + 'static {
        struct State<C> {
            address: String,
            next_id: Option<TransactionId>,
            client: C,
            next: bool,
        }

        stream::try_unfold(
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

                let next_id = if let Some(id) = state.next_id {
                    id
                } else {
                    let account_state = state.client.get_account_state(&state.address).await?;
                    let Some(tx_id) = account_state.last_transaction_id else {
                        return anyhow::Ok(None);
                    };

                    tx_id
                };

                let txs = state
                    .client
                    .get_transactions(&state.address, &next_id)
                    .await?;

                let next = txs.previous_transaction_id.is_some();
                anyhow::Ok(Some((
                    stream::iter(txs.transactions.into_iter().map(anyhow::Ok)),
                    State {
                        address: state.address,
                        next_id: txs.previous_transaction_id,
                        client: state.client,
                        next,
                    },
                )))
            },
        )
        .try_flatten()
    }

    fn get_account_tx_stream(
        &self,
        address: &str,
    ) -> impl Stream<Item = anyhow::Result<Transaction>> + Send + 'static {
        self.get_account_tx_stream_from(address, None)
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

        stream::try_unfold(
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
                    stream::iter(txs.transactions.into_iter().map(anyhow::Ok)),
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

        stream::try_unfold(
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
                    mode: 0,
                    account: t.address.clone(),
                    lt: t.transaction_id.lt,
                    hash: t.transaction_id.hash.clone(),
                });

                anyhow::Ok(Some((
                    stream::iter(txs.transactions.into_iter().map(anyhow::Ok)),
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
    ) -> impl Stream<Item = anyhow::Result<String>> + Send + 'static {
        let fwd = self.get_block_tx_id_stream(block, false).boxed();
        let rev = self.get_block_tx_id_stream(block, true).boxed();

        let merged = stream_select!(fwd.map(|r| (false, r)), rev.map(|r| (true, r)));

        try_stream! {
            let mut last: HashMap<bool, String> = HashMap::with_capacity(2);

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

    fn get_last_transaction_id(
        &self,
        address: &str,
        chain: i32,
        shard: i64,
        seqno: i32,
    ) -> impl std::future::Future<Output = anyhow::Result<TransactionId>> + Send {
        let client = self.clone();
        let address = address.to_owned();

        async move {
            let block = client.look_up_block_by_seqno(chain, shard, seqno).await?;
            let state = client.get_account_state_on_block(&address, block).await?;

            state
                .last_transaction_id
                .ok_or_else(|| anyhow::anyhow!("tx not found"))
        }
    }

    fn find_first_tx(
        &self,
        address: &str,
    ) -> impl std::future::Future<Output = anyhow::Result<TransactionId>> + Send {
        let client = self.clone();
        let address = address.to_owned();

        async move {
            let start = client.get_masterchain_info().await?.last;

            let length = start.seqno;
            let mut rhs = length;
            let mut lhs = 1;
            let mut cur = (lhs + rhs) / 2;

            let workchain = start.workchain;
            let shard = start.shard;

            let mut tx = client
                .get_last_transaction_id(&address, workchain, shard, cur)
                .await;

            while lhs < rhs {
                if tx.is_err() {
                    lhs = cur + 1;
                } else {
                    rhs = cur;
                }

                cur = (lhs + rhs) / 2;

                if cur == 0 {
                    break;
                }

                trace!("lhs: {}, rhs: {}, cur: {}", lhs, rhs, cur);

                tx = client
                    .get_last_transaction_id(&address, workchain, shard, cur)
                    .await;
            }

            let tx = tx?;

            trace!(tx = ?tx, "first tx");

            Ok(tx)
        }
    }

    fn get_account_tx_range_unordered(
        &self,
        address: &str,
        range: (Bound<TransactionId>, Bound<TransactionId>),
    ) -> impl std::future::Future<
        Output = anyhow::Result<BoxStream<'static, anyhow::Result<Transaction>>>,
    > + Send {
        let client = self.clone();
        let address = address.to_owned();

        async move {
            let (last_tx, first_tx) = futures::try_join!(
                async {
                    let last_tx = match range.start_bound().cloned() {
                        Bound::Included(tx) | Bound::Excluded(tx) => tx,
                        Bound::Unbounded => {
                            let state: AccountState = client.get_account_state(&address).await?;
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
                        Bound::Unbounded => client.find_first_tx(&address).await?,
                    };
                    anyhow::Ok(first_tx)
                }
            )?;

            let last_block = client
                .get_account_state_by_transaction(&address, last_tx.clone())
                .await?
                .block_id;
            let first_block = client
                .get_account_state_by_transaction(&address, first_tx.clone())
                .await?
                .block_id;

            let chunks = min(256, (last_block.seqno - first_block.seqno) / 28800);
            let step = (last_block.seqno - first_block.seqno) / chunks;

            let workchain = first_block.workchain;
            let shard = first_block.shard;
            let seqno = first_block.seqno;

            let mid: Vec<anyhow::Result<TransactionId>> = stream::iter(1..chunks)
                .map(|i| {
                    let client = client.clone();
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

            debug!(txs = ?txs);

            let streams = txs
                .windows(2)
                .map(|e| {
                    let [left, right, ..] = e else { unreachable!() };
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

                    client
                        .get_account_tx_range(&address, (right_bound, left_bound))
                        .boxed()
                })
                .collect_vec();

            Ok(Box::pin(stream::iter(streams).flatten_unordered(32))
                as BoxStream<'static, anyhow::Result<Transaction>>)
        }
    }

    fn get_account_tx_range(
        &self,
        address: &str,
        range: (Bound<TransactionId>, Bound<TransactionId>),
    ) -> impl Stream<Item = anyhow::Result<Transaction>> + Send + 'static {
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

impl<T: TonClient> TonClientExt for T {}
