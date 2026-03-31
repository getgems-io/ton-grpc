use async_stream::try_stream;
use futures::{Stream, TryStreamExt, stream};
use std::ops::{Bound, RangeBounds};

use crate::{TonClient, Transaction, TransactionId};

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
