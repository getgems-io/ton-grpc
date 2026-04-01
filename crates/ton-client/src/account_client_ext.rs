use crate::{AccountClient, AccountState, BlockClient, Transaction, TransactionId};
use anyhow::anyhow;
use async_stream::try_stream;
use futures::stream::BoxStream;
use futures::{Stream, StreamExt, TryStreamExt};
use itertools::Itertools;
use std::cmp::min;
use std::ops::{Bound, RangeBounds};
use ton_address::SmartContractAddress;
use tracing::{debug, trace};

pub trait AccountClientExt: BlockClient + AccountClient {
    fn get_account_tx_stream_from(
        &self,
        address: &SmartContractAddress,
        last_tx: Option<TransactionId>,
    ) -> impl Stream<Item = anyhow::Result<Transaction>> + Send + 'static {
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
                    futures::stream::iter(txs.transactions.into_iter().map(anyhow::Ok)),
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
        address: &SmartContractAddress,
    ) -> impl Stream<Item = anyhow::Result<Transaction>> + Send + 'static {
        self.get_account_tx_stream_from(address, None)
    }

    fn get_last_transaction_id(
        &self,
        address: &SmartContractAddress,
        chain: i32,
        shard: i64,
        seqno: i32,
    ) -> impl Future<Output = anyhow::Result<TransactionId>> + Send {
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
        address: &SmartContractAddress,
    ) -> impl Future<Output = anyhow::Result<TransactionId>> + Send {
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
        address: &SmartContractAddress,
        range: (Bound<TransactionId>, Bound<TransactionId>),
    ) -> impl Future<Output = anyhow::Result<BoxStream<'static, anyhow::Result<Transaction>>>> + Send
    {
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

            let mid: Vec<anyhow::Result<TransactionId>> = futures::stream::iter(1..chunks)
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

            Ok(
                Box::pin(futures::stream::iter(streams).flatten_unordered(32))
                    as BoxStream<'static, anyhow::Result<Transaction>>,
            )
        }
    }

    fn get_account_tx_range(
        &self,
        address: &SmartContractAddress,
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

impl<T: BlockClient + AccountClient> AccountClientExt for T {}
