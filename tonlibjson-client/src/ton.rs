use std::cmp::min;
use std::collections::Bound;
use std::ops::{RangeBounds};
use std::path::PathBuf;
use std::time::Duration;
use futures::{Stream, stream, TryStreamExt, StreamExt, try_join, TryStream};
use anyhow::anyhow;
use itertools::Itertools;
use serde_json::Value;
use tower::Layer;
use tower::buffer::Buffer;
use tower::load::PeakEwmaDiscover;
use tower::retry::budget::Budget;
use tower::retry::Retry;
use tracing::{instrument, trace};
use url::Url;
use crate::address::{InternalAccountAddress, ShardContextAccountAddress};
use crate::balance::{Balance, BalanceRequest};
use crate::block::{InternalTransactionId, RawTransaction, RawTransactions, MasterchainInfo, BlocksShards, BlockIdExt, AccountTransactionId, BlocksTransactions, ShortTxId, RawSendMessage, SmcStack, AccountAddress, BlocksGetTransactions, BlocksLookupBlock, BlockId, BlocksGetShards, BlocksGetBlockHeader, BlockHeader, RawGetTransactionsV2, RawGetAccountState, GetAccountState, GetMasterchainInfo, SmcMethodId, GetShardAccountCell, Cell, RawFullAccountState, WithBlock, RawGetAccountStateByTransaction, GetShardAccountCellByTransaction};
use crate::config::AppConfig;
use crate::discover::{ClientDiscover, CursorClientDiscover};
use crate::error::{ErrorLayer, ErrorService};
use crate::retry::RetryPolicy;
use crate::session::RunGetMethod;
use crate::request::Callable;

pub struct TonClient {
    client: ErrorService<Retry<RetryPolicy, Buffer<Balance, BalanceRequest>>>,
    last_block_receiver: tokio::sync::broadcast::Receiver<(BlockHeader, BlockHeader)>
}

impl Clone for TonClient {
    fn clone(&self) -> Self {
        Self {
            client: self.client.clone(),
            last_block_receiver: self.last_block_receiver.resubscribe()
        }
    }
}

const MAIN_CHAIN: i32 = -1;
const MAIN_SHARD: i64 = -9223372036854775808;

impl TonClient {
    pub async fn ready(&mut self) -> anyhow::Result<()> {
        self.get_block_header(0, MAIN_SHARD, 100).await?;

        Ok(())
    }

    pub async fn from_path(path: PathBuf) -> anyhow::Result<Self> {
        let client_discover = ClientDiscover::from_path(path).await?;
        let ewma_discover = PeakEwmaDiscover::new(
            client_discover,
            Duration::from_secs(15),
            Duration::from_secs(60),
            tower::load::CompleteOnResponse::default(),
        );
        let cursor_client_discover = CursorClientDiscover::new(ewma_discover);

        let client = Balance::new(cursor_client_discover);
        let last_block_receiver = client.last_block_receiver();

        let client = Buffer::new(client, 200000);
        let client = Retry::new(RetryPolicy::new(Budget::new(
            Duration::from_secs(10),
            10,
            0.1
        )), client);
        let client = ErrorLayer::default().layer(client);

        Ok(Self {
            client,
            last_block_receiver
        })
    }

    pub async fn from_url(url: Url, fallback_path: Option<PathBuf>) -> anyhow::Result<Self> {
        let client_discover = ClientDiscover::new(
            url,
            Duration::from_secs(60),
            fallback_path
        ).await?;
        let ewma_discover = PeakEwmaDiscover::new(
            client_discover,
            Duration::from_secs(15),
            Duration::from_secs(60),
            tower::load::CompleteOnResponse::default(),
        );
        let cursor_client_discover = CursorClientDiscover::new(ewma_discover);

        let client = Balance::new(cursor_client_discover);
        let last_block_receiver = client.last_block_receiver();

        let client = Buffer::new(client, 200000);
        let client = Retry::new(RetryPolicy::new(Budget::new(
            Duration::from_secs(10),
            10,
            0.1
        )), client);
        let client = ErrorLayer::default().layer(client);

        Ok(Self {
            client,
            last_block_receiver
        })
    }

    pub async fn from_env() -> anyhow::Result<Self> {
        let config = AppConfig::from_env()?;

        tracing::warn!("Ton config url: {}", config.config_url);
        tracing::warn!("Ton config fallback path: {:?}", config.config_path);

        Self::from_url(config.config_url, config.config_path).await
    }

    pub async fn get_masterchain_info(&self) -> anyhow::Result<MasterchainInfo> {
        let mut client = self.client.clone();

        GetMasterchainInfo::default()
            .call(&mut client)
            .await
    }

    pub async fn look_up_block_by_seqno(
        &self,
        chain: i32,
        shard: i64,
        seqno: i32,
    ) -> anyhow::Result<BlockIdExt> {
        if seqno <= 0 {
            return Err(anyhow!("seqno must be greater than 0"));
        }

        let mut client = self.client.clone();

        BlocksLookupBlock::seqno(BlockId::new(chain, shard, seqno))
            .call(&mut client)
            .await
    }

    pub async fn look_up_block_by_lt(
        &self,
        chain: i32,
        shard: i64,
        lt: i64,
    ) -> anyhow::Result<BlockIdExt> {
        if lt <= 0 {
            return Err(anyhow!("lt must be greater than 0"));
        }

        let mut client = self.client.clone();

        BlocksLookupBlock::logical_time(BlockId::new(chain, shard, 0), lt)
            .call(&mut client)
            .await
    }

    pub async fn get_shards(&self, master_seqno: i32) -> anyhow::Result<BlocksShards> {
        let block = self
            .look_up_block_by_seqno(MAIN_CHAIN, MAIN_SHARD, master_seqno)
            .await?;

        let mut client = self.client.clone();

        BlocksGetShards::new(block).call(&mut client).await
    }

    pub async fn get_shards_by_block_id(&self, block_id: BlockIdExt) -> anyhow::Result<Vec<BlockIdExt>> {
        if block_id.workchain != -1 {
            return Err(anyhow!("workchain must be -1"))
        }

        let mut client = self.client.clone();

        Ok(BlocksGetShards::new(block_id).call(&mut client).await?.shards)
    }

    pub async fn get_block_header(
        &self,
        chain: i32,
        shard: i64,
        seqno: i32,
    ) -> anyhow::Result<BlockHeader> {
        let id = self.look_up_block_by_seqno(chain, shard, seqno).await?;

        let mut client = self.client.clone();

        BlocksGetBlockHeader::new(id).call(&mut client).await
    }

    #[instrument(skip_all, err)]
    pub async fn raw_get_account_state(&self, address: &str) -> anyhow::Result<RawFullAccountState> {
        let mut client = self.client.clone();

        let account_address = AccountAddress::new(address)?;

        RawGetAccountState::new(account_address)
            .call(&mut client)
            .await
    }

    pub async fn raw_get_account_state_on_block(&self, address: &str, block_id: BlockIdExt) -> anyhow::Result<RawFullAccountState> {
        let mut client = self.client.clone();

        let account_address = AccountAddress::new(address)?;

        WithBlock::new(block_id, RawGetAccountState::new(account_address))
            .call(&mut client)
            .await
    }

    #[instrument(skip_all, err)]
    pub async fn raw_get_account_state_by_transaction(&self, address: &str, transaction_id: InternalTransactionId) -> anyhow::Result<RawFullAccountState> {
        let mut client = self.client.clone();

        let account_address = AccountAddress::new(address)?;

        RawGetAccountStateByTransaction::new(account_address, transaction_id)
            .call(&mut client)
            .await
    }

    pub async fn get_account_state(&self, address: &str) -> anyhow::Result<Value> {
        let mut client = self.client.clone();

        let account_address = AccountAddress::new(address)?;

        GetAccountState::new(account_address)
            .call(&mut client)
            .await
    }

    pub async fn raw_get_transactions(
        &self,
        address: &str,
        from_tx: &InternalTransactionId
    ) -> anyhow::Result<RawTransactions> {
        let mut client = self.client.clone();

        let address = AccountAddress::new(address)?;
        let request = RawGetTransactionsV2::new(
            address,
            from_tx.clone()
        );

        request.call(&mut client).await
    }

    pub async fn blocks_get_transactions(
        &self,
        block: &BlockIdExt,
        tx: Option<AccountTransactionId>,
        reverse: bool,
        count: i32
    ) -> anyhow::Result<BlocksTransactions> {
        let mut client = self.client.clone();

        BlocksGetTransactions::unverified(
            block.to_owned(),
            tx,
            reverse,
            count
        ).call(&mut client).await
    }

    pub async fn blocks_get_transactions_verified(
        &self,
        block: &BlockIdExt,
        tx: Option<AccountTransactionId>,
        reverse: bool,
        count: i32
    ) -> anyhow::Result<BlocksTransactions> {
        let mut client = self.client.clone();

        BlocksGetTransactions::verified(
            block.to_owned(),
            tx,
            reverse,
            count
        ).call(&mut client).await
    }

    pub async fn send_message(&self, message: &str) -> anyhow::Result<Value> {
        let mut client = self.client.clone();

        RawSendMessage::new(message.to_string()).call(&mut client).await
    }

    pub fn get_block_tx_stream_unordered(&self, block: &BlockIdExt) -> impl Stream<Item=anyhow::Result<ShortTxId>> + 'static {
        let lstream = self.get_block_tx_stream(&block, false).into_stream();
        let rstream = self.get_block_tx_stream(&block, true).into_stream();

        async_stream::try_stream! {
            tokio::pin!(lstream);
            tokio::pin!(rstream);

            let mut last_ltx = None;
            let mut last_rtx = None;

            loop {
                tokio::select! {
                    Some(tx) = lstream.next() => {
                        match tx {
                            Err(e) => yield Err(e)?,
                            Ok(tx) => {
                                if let Some(ref rtx) = last_rtx {
                                    if rtx == &tx {
                                        return;
                                    }
                                }
                                last_ltx.replace(tx.clone());
                                yield tx
                            }
                        }
                    },
                    Some(tx) = rstream.next() => {
                        match tx {
                            Err(e) => yield Err(e)?,
                            Ok(tx) => {
                                if let Some(ref ltx) = last_ltx {
                                    if ltx == &tx {
                                        return;
                                    }
                                }
                                last_rtx.replace(tx.clone());
                                yield tx
                            }
                        }
                    },
                    else => return
                }
            }
        }
    }

    pub fn get_block_tx_stream(
        &self,
        block: &BlockIdExt,
        reverse: bool
    ) -> impl Stream<Item=anyhow::Result<ShortTxId>> + 'static {
        struct State {
            last_tx: Option<AccountTransactionId>,
            incomplete: bool,
            block: BlockIdExt,
            this: TonClient,
            exp: u32
        }

        stream::try_unfold(
            State {
                last_tx: None,
                incomplete: true,
                block: block.clone(),
                this: self.clone(),
                exp: 5
            },
            move |state| {
                async move {
                    if !state.incomplete {
                        return anyhow::Ok(None);
                    }

                    let txs = state.this.blocks_get_transactions(&state.block, state.last_tx, reverse, 2_i32.pow(state.exp)).await?;

                    tracing::debug!("got {} transactions", txs.transactions.len());

                    let last_tx = txs.transactions.last().map(Into::into);

                    anyhow::Ok(Some((
                        stream::iter(txs.transactions.into_iter().map(anyhow::Ok)),
                        State {
                            last_tx,
                            incomplete: txs.incomplete,
                            block: state.block,
                            this: state.this,
                            exp: min(8, state.exp + 1)
                        },
                    )))
                }
            },
        )
            .try_flatten()
    }

    pub fn get_account_tx_stream(
        &self,
        address: &str,
    ) -> impl Stream<Item = anyhow::Result<RawTransaction>> + 'static {
        self.get_account_tx_stream_from(address, None)
    }

    // TODO[akostylev0] run search of first tx in parallel with `range` stream
    pub async fn get_account_tx_range_unordered<R : RangeBounds<InternalTransactionId> + 'static>(
        &self,
        address: &str,
        range: R
    ) -> anyhow::Result<impl Stream<Item = anyhow::Result<RawTransaction>> + 'static> {
        let ((last_block, last_tx),
            (first_block, first_tx)) = try_join!(async {
                let last_tx = match range.start_bound().cloned() {
                    Bound::Included(tx) | Bound::Excluded(tx) => tx.to_owned(),
                    Bound::Unbounded => {
                        let state = self.raw_get_account_state(address).await?;

                        state.last_transaction_id.ok_or_else(|| anyhow!("invalid last tx"))?
                    },
                };
                let last_block = self.raw_get_account_state_by_transaction(address, last_tx.clone()).await?.block_id;

                anyhow::Ok((last_block, last_tx))
            }, async {
                let first_tx = match range.end_bound().cloned() {
                    Bound::Included(tx) | Bound::Excluded(tx) => tx.to_owned(),
                    Bound::Unbounded => self.find_first_tx(address).await?,
                };
                let first_block = self.raw_get_account_state_by_transaction(address, first_tx.clone()).await?.block_id;

                anyhow::Ok((first_block, first_tx))
            }
        )?;

        let chunks = min(256, (last_block.seqno - first_block.seqno) / 28800);
        let step = (last_block.seqno - first_block.seqno) / chunks;

        let workchain = first_block.workchain;
        let shard = first_block.shard;
        let seqno = first_block.seqno;

        let mid: Vec<anyhow::Result<InternalTransactionId>> = stream::iter(1..chunks)
            .map(|i| async move {
                let block = self.look_up_block_by_seqno(workchain, shard, seqno + step * i).await?;
                let state = self.raw_get_account_state_on_block(address, block).await?;

                anyhow::Ok(state.last_transaction_id.ok_or(anyhow!("invalid last tx"))?)
            }).buffered(32).collect().await;

        let mut mid = mid.into_iter()
            .collect::<anyhow::Result<Vec<InternalTransactionId>>>()?;

        let mut txs = vec![first_tx.clone()];
        txs.append(&mut mid);
        txs.push(last_tx.clone());
        txs.dedup();

        tracing::debug!(txs = ?txs);

        let streams = txs.windows(2).to_owned().map(|e| {
            let [left, right, ..] = e else {
                unreachable!()
            };
            let left_bound = if left == &first_tx { range.end_bound().cloned() } else { Bound::Included(left.clone()) };
            let right_bound = if right == &last_tx { range.start_bound().cloned() } else { Bound::Excluded(right.clone()) };

            self.get_account_tx_range(address, (right_bound, left_bound)).boxed()
        }).collect_vec();

        Ok(stream::iter(streams).flatten_unordered(32))
    }

    pub fn get_account_tx_range<R : RangeBounds<InternalTransactionId> + 'static>(
        &self,
        address: &str,
        range: R
    ) -> impl Stream<Item = anyhow::Result<RawTransaction>> + 'static {
        let last_tx = match range.start_bound() {
            Bound::Included(tx) | Bound::Excluded(tx) => Some(tx.to_owned()),
            Bound::Unbounded => None,
        };
        let stream = self.get_account_tx_stream_from(address, last_tx);

        let exclude = if let Bound::Excluded(tx) = range.start_bound().cloned() { Some(tx) } else { None };
        let stream = stream.try_skip_while(move |sx| std::future::ready(
            if let Some(tx) = exclude.as_ref() {
                Ok(tx == &sx.transaction_id)
            } else { Ok(false) }
        ));

        let end = range.end_bound().cloned();
        let mut found = false;
        stream.try_take_while(move |x| std::future::ready(Ok({
            match end.as_ref() {
                Bound::Unbounded => true,
                Bound::Included(tx) => {
                    if tx == &x.transaction_id {
                        found = true;

                        true
                    } else {
                        !found
                    }
                },
                Bound::Excluded(tx) => {
                    if tx == &x.transaction_id {
                        found = true;

                        false
                    } else {
                        !found
                    }
                }
            }
        })))
    }

    pub fn get_account_tx_stream_from(
        &self,
        address: &str,
        last_tx: Option<InternalTransactionId>,
    ) -> impl Stream<Item = anyhow::Result<RawTransaction>> + 'static {
        struct State {
            address: String,
            next_id: Option<InternalTransactionId>,
            this: TonClient,
            next: bool
        }

        stream::try_unfold(State { address: address.to_owned(), next_id: last_tx, this: self.clone(), next: true }, move |state| async move {
            if !state.next {
                return anyhow::Ok(None);
            }

            let next_id = if let Some(id) = state.next_id { id } else {
                let state = state.this.raw_get_account_state(&state.address).await?;

                state.last_transaction_id.ok_or(anyhow!("transaction_id invalid"))?
            };

            let txs = state.this
                .raw_get_transactions(&state.address, &next_id)
                .await?;

            let items = txs.transactions;

            let next = txs.previous_transaction_id.is_some();
            anyhow::Ok(Some((
                stream::iter(items.into_iter().map(anyhow::Ok)),
                State {
                    address: state.address,
                    next_id: txs.previous_transaction_id,
                    this: state.this,
                    next
                }
            )))
        }).try_flatten()
    }

    pub async fn run_get_method(&self, address: String, method: String, stack: SmcStack) -> anyhow::Result<Value> {
        let address = AccountAddress::new(&address)?;
        let method = SmcMethodId::new_name(method);
        let mut client = self.client.clone();

        RunGetMethod::new(address, method, stack)
            .call(&mut client)
            .await
    }

    pub async fn get_shard_account_cell(&self, address: &str) -> anyhow::Result<Cell> {
        let address = AccountAddress::new(address)?;

        GetShardAccountCell::new(address)
            .call(&mut self.client.clone())
            .await
    }

    pub async fn get_shard_account_cell_on_block(&self, address: &str, block: BlockIdExt) -> anyhow::Result<Cell> {
        let address = AccountAddress::new(address)?;

        WithBlock::new(block, GetShardAccountCell::new(address))
            .call(&mut self.client.clone())
            .await
    }

    pub async fn get_shard_account_cell_by_transaction(&self, address: &str, transaction: InternalTransactionId) -> anyhow::Result<Cell> {
        let address = AccountAddress::new(address)?;

        GetShardAccountCellByTransaction::new(address, transaction)
            .call(&mut self.client.clone())
            .await
    }

    pub fn last_block_stream(&self) -> impl Stream<Item=(BlockHeader, BlockHeader)> {
        tokio_stream::wrappers::BroadcastStream::new(self.last_block_receiver.resubscribe())
            .filter_map(|r| async {
                match r {
                    Ok(v) => Some(v),
                    Err(e) => { tracing::error!("{}", e); None }
                }
            })
    }

    pub fn get_accounts_in_block_stream(&self, block: &BlockIdExt) -> impl TryStream<Ok=InternalAccountAddress, Error=anyhow::Error> + 'static {
        let block = block.clone();
        let lstream = self.get_block_tx_stream(&block, false).into_stream();
        let rstream = self.get_block_tx_stream(&block, true).into_stream();

        let stream = async_stream::try_stream! {
            tokio::pin!(lstream);
            tokio::pin!(rstream);

            let mut last_laddr: Option<ShardContextAccountAddress> = None;
            let mut last_raddr: Option<ShardContextAccountAddress> = None;

            loop {
                tokio::select! {
                    Some(tx) = lstream.next() => {
                        match tx {
                            Err(e) => yield Err(e)?,
                            Ok(tx) => {
                                if let Some(ref raddr) = last_raddr {
                                    if raddr == &tx.account {
                                        return;
                                    }
                                }

                                if let Some(ref laddr) = last_laddr {
                                    if laddr == &tx.account {
                                        continue;
                                    }
                                }

                                last_laddr.replace(tx.account.clone());
                                yield tx.account.clone();
                            }
                        }
                    },
                    Some(tx) = rstream.next() => {
                        match tx {
                            Err(e) => yield Err(e)?,
                            Ok(tx) => {
                                if let Some(ref laddr) = last_laddr {
                                    if laddr == &tx.account {
                                        return;
                                    }
                                }

                                if let Some(ref raddr) = last_raddr {
                                    if raddr == &tx.account {
                                        continue;
                                    }
                                }

                                last_raddr.replace(tx.account.clone());
                                yield tx.account.clone();
                            }
                        }
                    },
                    else => return
                }
            }
        };

        stream.map_ok(move |a: ShardContextAccountAddress| a.into_internal(block.workchain))
    }

    async fn find_first_tx(&self, account: &str) -> anyhow::Result<InternalTransactionId> {
        let start = self.get_masterchain_info().await?.last;

        let length = start.seqno;
        let mut rhs = length;
        let mut lhs = 1;
        let mut cur = (lhs + rhs) / 2;

        let workchain = start.workchain;
        let shard = start.shard;

        let mut tx = self.check_account_available(account, &BlockId::new(workchain, shard, cur)).await;

        while lhs < rhs {
            // TODO[akostylev0] specify error
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

            tx = self.check_account_available(account, &BlockId::new(workchain, shard, cur)).await;
        }

        let tx = tx?;

        trace!(tx = ?tx, "first tx");

        Ok(tx)
    }

    async fn check_account_available(&self, account: &str, block: &BlockId) -> anyhow::Result<InternalTransactionId> {
        let block = self
            .look_up_block_by_seqno(block.workchain, block.shard, block.seqno).await?;
        let state = self
            .raw_get_account_state_on_block(account, block).await?;

        state.last_transaction_id.ok_or(anyhow!("tx not found"))
    }
}
