use std::cmp::min;
use std::collections::HashMap;
use std::future::Future;
use std::ops::Bound;

use async_stream::try_stream;
use futures::stream::BoxStream;
use futures::{StreamExt, TryStreamExt, stream, try_join};
use itertools::Itertools;
use tokio_stream::StreamMap;
use tracing::trace;

use crate::types::{
    BlockIdExt, BlocksHeader, InternalAccountAddress, InternalTransactionId, MasterchainInfo,
    RawFullAccountState, RawTransaction, ShortTxId, TvmCell,
};

pub trait TonClient: Clone + Send + Sync + 'static {
    type Error: std::fmt::Display + From<anyhow::Error> + Send + Sync + 'static;

    fn ready(&mut self) -> impl Future<Output = Result<(), Self::Error>> + Send {
        async move {
            self.get_masterchain_info().await?;

            Ok(())
        }
    }

    fn get_masterchain_info(
        &self,
    ) -> impl Future<Output = Result<MasterchainInfo, Self::Error>> + Send;

    fn look_up_block_by_seqno(
        &self,
        workchain: i32,
        shard: i64,
        seqno: i32,
    ) -> impl Future<Output = Result<BlockIdExt, Self::Error>> + Send;

    fn get_block_header(
        &self,
        workchain: i32,
        shard: i64,
        seqno: i32,
        hashes: Option<(String, String)>,
    ) -> impl Future<Output = Result<BlocksHeader, Self::Error>> + Send;

    fn get_shards_by_block_id(
        &self,
        block_id: BlockIdExt,
    ) -> impl Future<Output = Result<Vec<BlockIdExt>, Self::Error>> + Send;

    fn raw_get_account_state_on_block(
        &self,
        address: &str,
        block_id: BlockIdExt,
    ) -> impl Future<Output = Result<RawFullAccountState, Self::Error>> + Send;

    fn raw_get_account_state_at_least_block(
        &self,
        address: &str,
        block_id: &BlockIdExt,
    ) -> impl Future<Output = Result<RawFullAccountState, Self::Error>> + Send;

    fn raw_get_account_state_by_transaction(
        &self,
        address: &str,
        transaction_id: InternalTransactionId,
    ) -> impl Future<Output = Result<RawFullAccountState, Self::Error>> + Send;

    fn raw_get_account_state(
        &self,
        address: &str,
    ) -> impl Future<Output = Result<RawFullAccountState, Self::Error>> + Send;

    fn get_block_transactions_batch(
        &self,
        block: &BlockIdExt,
        after: Option<&ShortTxId>,
        reverse: bool,
        limit: i32,
    ) -> impl Future<Output = Result<(Vec<ShortTxId>, bool), Self::Error>> + Send;

    fn get_block_raw_transactions_batch(
        &self,
        block: &BlockIdExt,
        after: Option<&ShortTxId>,
        reverse: bool,
        limit: i32,
    ) -> impl Future<Output = Result<(Vec<RawTransaction>, bool), Self::Error>> + Send;

    fn get_account_transactions_batch(
        &self,
        address: &str,
        from: &InternalTransactionId,
        limit: i32,
    ) -> impl Future<
        Output = Result<(Vec<RawTransaction>, Option<InternalTransactionId>), Self::Error>,
    > + Send;

    fn get_shard_account_cell_on_block(
        &self,
        address: &str,
        block: BlockIdExt,
    ) -> impl Future<Output = Result<TvmCell, Self::Error>> + Send;

    fn get_shard_account_cell_at_least_block(
        &self,
        address: &str,
        block_id: &BlockIdExt,
    ) -> impl Future<Output = Result<TvmCell, Self::Error>> + Send;

    fn send_message_returning_hash(
        &self,
        body: &str,
    ) -> impl Future<Output = Result<String, Self::Error>> + Send;

    fn get_account_tx_range_unordered(
        &self,
        address: &str,
        range: (Bound<InternalTransactionId>, Bound<InternalTransactionId>),
    ) -> impl Future<
        Output = Result<BoxStream<'static, Result<RawTransaction, Self::Error>>, Self::Error>,
    > + Send {
        let this = self.clone();
        let address = address.to_owned();

        async move {
            let (start_bound, end_bound) = range;
            let start_bound_for_last = start_bound.clone();
            let end_bound_for_first = end_bound.clone();
            let this_for_last = this.clone();
            let address_for_last = address.clone();
            let this_for_first = this.clone();
            let address_for_first = address.clone();

            let ((last_block, last_tx), (first_block, first_tx)) = try_join!(
                async move {
                    let last_tx = match start_bound_for_last {
                        Bound::Included(tx) | Bound::Excluded(tx) => tx,
                        Bound::Unbounded => {
                            let state = this_for_last
                                .raw_get_account_state(&address_for_last)
                                .await?;

                            state
                                .last_transaction_id
                                .ok_or_else(|| anyhow::anyhow!("invalid last tx"))?
                        }
                    };
                    let last_block = this_for_last
                        .raw_get_account_state_by_transaction(&address_for_last, last_tx.clone())
                        .await?
                        .block_id;

                    Ok::<_, Self::Error>((last_block, last_tx))
                },
                async move {
                    let first_tx = match end_bound_for_first {
                        Bound::Included(tx) | Bound::Excluded(tx) => tx,
                        Bound::Unbounded => {
                            find_first_tx(&this_for_first, &address_for_first).await?
                        }
                    };
                    let first_block = this_for_first
                        .raw_get_account_state_by_transaction(&address_for_first, first_tx.clone())
                        .await?
                        .block_id;

                    Ok::<_, Self::Error>((first_block, first_tx))
                }
            )?;

            let chunks = min(256, (last_block.seqno - first_block.seqno) / 28800);
            let step = (last_block.seqno - first_block.seqno) / chunks;

            let workchain = first_block.workchain;
            let shard = first_block.shard;
            let seqno = first_block.seqno;

            let mid: Vec<Result<InternalTransactionId, Self::Error>> = stream::iter(1..chunks)
                .map(|i| {
                    let this = this.clone();
                    let address = address.clone();

                    async move {
                        let block = this
                            .look_up_block_by_seqno(workchain, shard, seqno + step * i)
                            .await?;
                        let state = this.raw_get_account_state_on_block(&address, block).await?;

                        state
                            .last_transaction_id
                            .ok_or_else(|| anyhow::anyhow!("invalid last tx").into())
                    }
                })
                .buffered(32)
                .collect()
                .await;

            let mut mid = mid
                .into_iter()
                .collect::<Result<Vec<InternalTransactionId>, _>>()?;

            let mut txs = vec![first_tx.clone()];
            txs.append(&mut mid);
            txs.push(last_tx.clone());
            txs.dedup();

            tracing::debug!(txs = ?txs);

            let streams = txs
                .windows(2)
                .to_owned()
                .map(|items| {
                    let [left, right, ..] = items else {
                        unreachable!()
                    };
                    let left_bound = if left == &first_tx {
                        end_bound.clone()
                    } else {
                        Bound::Included(left.clone())
                    };
                    let right_bound = if right == &last_tx {
                        start_bound.clone()
                    } else {
                        Bound::Excluded(right.clone())
                    };

                    this.get_account_tx_range(&address, (right_bound, left_bound))
                        .boxed()
                })
                .collect_vec();

            Ok(stream::iter(streams).flatten_unordered(32).boxed())
        }
    }

    fn get_account_tx_range(
        &self,
        address: &str,
        range: (Bound<InternalTransactionId>, Bound<InternalTransactionId>),
    ) -> BoxStream<'static, Result<RawTransaction, Self::Error>> {
        let (start_bound, end_bound) = range;
        let last_tx = match start_bound.clone() {
            Bound::Included(tx) | Bound::Excluded(tx) => Some(tx),
            Bound::Unbounded => None,
        };
        let stream = get_account_tx_stream_from(self.clone(), address.to_owned(), last_tx);

        let exclude = if let Bound::Excluded(tx) = start_bound {
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

        let end = end_bound;
        try_stream! {
            tokio::pin!(stream);
            while let Some(item) = stream.try_next().await? {
                match end.as_ref() {
                    Bound::Unbounded => {
                        yield item;
                    }
                    Bound::Included(tx) => {
                        let is_end = tx == &item.transaction_id;
                        yield item;
                        if is_end {
                            break;
                        }
                    }
                    Bound::Excluded(tx) => {
                        if tx == &item.transaction_id {
                            break;
                        }
                        yield item;
                    }
                }
            }
        }
        .boxed()
    }

    fn get_block_tx_stream_unordered(
        &self,
        block: &BlockIdExt,
    ) -> BoxStream<'static, Result<ShortTxId, Self::Error>> {
        let stream_map = StreamMap::from_iter(
            [false, true].map(|reverse| (reverse, self.get_block_tx_id_stream(block, reverse))),
        );

        try_stream! {
            let mut last = HashMap::with_capacity(2);

            for await (key, tx) in stream_map {
                let tx = tx?;

                if let Some(prev_tx) = last.get(&!key)
                    && prev_tx == &tx
                {
                    return;
                }

                last.insert(key, tx.clone());
                yield tx;
            }
        }
        .boxed()
    }

    fn get_block_tx_id_stream(
        &self,
        block: &BlockIdExt,
        reverse: bool,
    ) -> BoxStream<'static, Result<ShortTxId, Self::Error>> {
        let block = block.clone();
        let this = self.clone();

        try_stream! {
            let mut after: Option<ShortTxId> = None;

            loop {
                let (txs, has_more) = this
                    .get_block_transactions_batch(&block, after.as_ref(), reverse, 256)
                    .await?;

                after = txs.last().cloned();

                for tx in txs {
                    yield tx;
                }

                if !has_more {
                    break;
                }
            }
        }
        .boxed()
    }

    fn get_block_tx_stream(
        &self,
        block: &BlockIdExt,
        reverse: bool,
    ) -> BoxStream<'static, Result<RawTransaction, Self::Error>> {
        let block = block.clone();
        let this = self.clone();

        try_stream! {
            let mut after: Option<ShortTxId> = None;

            loop {
                let (txs, has_more) = this
                    .get_block_raw_transactions_batch(&block, after.as_ref(), reverse, 256)
                    .await?;

                for tx in txs {
                    yield tx;
                }

                if !has_more {
                    break;
                }

                let (next_batch, _) = this
                    .get_block_transactions_batch(&block, after.as_ref(), reverse, 256)
                    .await?;

                after = next_batch.last().cloned();
                if after.is_none() {
                    break;
                }
            }
        }
        .boxed()
    }

    fn get_accounts_in_block_stream(
        &self,
        block: &BlockIdExt,
    ) -> BoxStream<'static, Result<InternalAccountAddress, Self::Error>>;
}

fn get_account_tx_stream_from<T: TonClient>(
    this: T,
    address: String,
    last_tx: Option<InternalTransactionId>,
) -> BoxStream<'static, Result<RawTransaction, T::Error>> {
    struct State<TonClientImpl> {
        address: String,
        next_id: Option<InternalTransactionId>,
        this: TonClientImpl,
        next: bool,
    }

    stream::try_unfold(
        State {
            address,
            next_id: last_tx,
            this,
            next: true,
        },
        move |state| async move {
            if !state.next {
                return Ok::<_, T::Error>(None);
            }

            let next_id = if let Some(id) = state.next_id {
                id
            } else {
                let state = state.this.raw_get_account_state(&state.address).await?;
                let Some(tx_id) = state.last_transaction_id else {
                    return Ok::<_, T::Error>(None);
                };

                tx_id
            };

            let (items, previous_transaction_id) = state
                .this
                .get_account_transactions_batch(&state.address, &next_id, 16)
                .await?;
            let next = previous_transaction_id.is_some();

            Ok::<_, T::Error>(Some((
                stream::iter(items.into_iter().map(Ok)),
                State {
                    address: state.address,
                    next_id: previous_transaction_id,
                    this: state.this,
                    next,
                },
            )))
        },
    )
    .try_flatten()
    .boxed()
}

async fn find_first_tx<T: TonClient>(
    client: &T,
    account: &str,
) -> Result<InternalTransactionId, T::Error> {
    let start = client.get_masterchain_info().await?.last;

    let mut rhs = start.seqno;
    let mut lhs = 1;
    let mut cur = (lhs + rhs) / 2;

    let workchain = start.workchain;
    let shard = start.shard;

    let mut tx = check_account_available(client, account, workchain, shard, cur).await;

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

        tx = check_account_available(client, account, workchain, shard, cur).await;
    }

    let tx = tx?;

    trace!(tx = ?tx, "first tx");

    Ok(tx)
}

async fn check_account_available<T: TonClient>(
    client: &T,
    account: &str,
    workchain: i32,
    shard: i64,
    seqno: i32,
) -> Result<InternalTransactionId, T::Error> {
    let block = client
        .look_up_block_by_seqno(workchain, shard, seqno)
        .await?;
    let state = client
        .raw_get_account_state_on_block(account, block)
        .await?;

    state
        .last_transaction_id
        .ok_or_else(|| anyhow::anyhow!("tx not found").into())
}
