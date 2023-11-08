use std::cmp::min;
use std::collections::{Bound, HashMap};
use std::future::IntoFuture;
use std::ops::{RangeBounds};
use std::path::PathBuf;
use std::time::Duration;
use futures::{Stream, stream, TryStreamExt, StreamExt, try_join, TryStream, TryFutureExt, FutureExt};
use anyhow::anyhow;
use async_stream::try_stream;
use futures::future::BoxFuture;
use itertools::Itertools;
use serde_json::Value;
use tokio_stream::StreamMap;
use tower::load::PeakEwmaDiscover;
use tower::retry::budget::Budget;
use tower::retry::Retry;
use tower::ServiceExt;
use tower::timeout::Timeout;
use tracing::{instrument, trace};
use url::Url;
use crate::address::{InternalAccountAddress, ShardContextAccountAddress};
use crate::balance::{Balance, BlockCriteria, Route, Router};
use crate::block::{InternalTransactionId, RawTransaction, RawTransactions, MasterchainInfo, BlocksShards, BlockIdExt, AccountTransactionId, BlocksTransactions, ShortTxId, RawSendMessage, SmcStack, AccountAddress, BlocksGetTransactions, BlocksLookupBlock, BlockId, BlocksGetShards, BlocksGetBlockHeader, BlockHeader, RawGetTransactionsV2, RawGetAccountState, GetAccountState, GetMasterchainInfo, SmcMethodId, GetShardAccountCell, Cell, RawFullAccountState, WithBlock, RawGetAccountStateByTransaction, GetShardAccountCellByTransaction, RawSendMessageReturnHash};
use crate::config::{AppConfig, default_ton_config_url};
use crate::discover::{ClientDiscover, CursorClientDiscover};
use crate::error::ErrorService;
use crate::helper::Side;
use crate::request::{Forward, Specialized};
use crate::retry::RetryPolicy;
use crate::session::RunGetMethod;
use crate::shared::SharedService;

#[derive(Clone)]
pub struct TonClient {
    client: ErrorService<Timeout<Retry<RetryPolicy, SharedService<Balance>>>>
}

const MAIN_CHAIN: i32 = -1;
const MAIN_SHARD: i64 = -9223372036854775808;

enum ConfigSource {
    FromFile { path: PathBuf },
    FromUrl { url: Url, interval: Duration, fallback_path: Option<PathBuf> },
}

pub struct TonClientBuilder {
    config_source: ConfigSource,
    timeout: Duration,
    ewma_default_rtt: Duration,
    ewma_decay: Duration,
    retry_budget_ttl: Duration,
    retry_min_per_sec: u32,
    retry_percent: f32,
    retry_first_delay: Duration,
    retry_max_delay: Duration
}

impl Default for TonClientBuilder {
    fn default() -> Self {
        Self {
            config_source: ConfigSource::FromUrl { url: default_ton_config_url(), interval: Duration::from_secs(60), fallback_path: None },
            timeout: Duration::from_secs(10),
            ewma_default_rtt: Duration::from_millis(70),
            ewma_decay: Duration::from_millis(1),
            retry_budget_ttl: Duration::from_secs(10),
            retry_min_per_sec: 10,
            retry_percent: 0.1,
            retry_first_delay: Duration::from_millis(128),
            retry_max_delay: Duration::from_millis(4096)
        }
    }
}

impl TonClientBuilder {
    pub fn from_config_path(path: PathBuf) -> Self {
        Self {
            config_source: ConfigSource::FromFile { path },
            .. Default::default()
        }
    }

    pub fn from_config_url(url: Url, interval: Duration) -> Self {
        Self {
            config_source: ConfigSource::FromUrl { url, interval, fallback_path: None },
            .. Default::default()
        }
    }

    pub fn set_config_url<U: TryInto<Url>>(mut self, url: U, interval: Duration) -> Result<Self, U::Error> {
        self.config_source = ConfigSource::FromUrl { url: url.try_into()?, interval, fallback_path: None };

        Ok(self)
    }

    pub fn from_config_url_with_fallback(url: Url, interval: Duration, fallback_path: Option<PathBuf>) -> Self {
        Self {
            config_source: ConfigSource::FromUrl { url, interval, fallback_path },
            .. Default::default()
        }
    }

    pub fn set_ewma_default_rtt(mut self, default_rtt: Duration) -> Self {
        self.ewma_default_rtt = default_rtt;

        self
    }

    pub fn set_ewma_decay(mut self, decay: Duration) -> Self {
        self.ewma_decay = decay;

        self
    }

    pub fn set_retry_budget_ttl(mut self, budget_ttl: Duration) -> Self {
        self.retry_budget_ttl = budget_ttl;

        self
    }

    pub fn set_retry_min_per_sec(mut self, retry_min_per_sec: u32) -> Self {
        self.retry_min_per_sec = retry_min_per_sec;

        self
    }

    pub fn set_retry_percent(mut self, retry_percent: f32) -> Self {
        self.retry_percent = retry_percent;

        self
    }

    pub fn set_retry_first_delay(mut self, first_delay: Duration) -> Self {
        self.retry_first_delay = first_delay;

        self
    }

    pub fn set_retry_max_delay(mut self, delay: Duration) -> Self {
        self.retry_max_delay = delay;

        self
    }

    pub fn set_timeout(mut self, timeout: Duration) -> Self {
        self.timeout = timeout;

        self
    }

    pub async fn build(self) -> anyhow::Result<TonClient> {
        let client_discover = match self.config_source {
            ConfigSource::FromFile { path } => { ClientDiscover::from_path(path).await? }
            ConfigSource::FromUrl { url, interval, fallback_path } => { ClientDiscover::new(url, interval, fallback_path).await? }
        };

        let ewma_discover = PeakEwmaDiscover::new::<Value>(
            client_discover,
            self.ewma_default_rtt,
            self.ewma_decay,
            tower::load::CompleteOnResponse::default(),
        );

        let cursor_client_discover = CursorClientDiscover::new(ewma_discover);

        let router = Router::new(cursor_client_discover);
        let client = Balance::new(router);

        let client = SharedService::new(client);
        let client = Retry::new(RetryPolicy::new(Budget::new(
            self.retry_budget_ttl,
            self.retry_min_per_sec,
            self.retry_percent
        ), self.retry_first_delay.as_millis() as u64, self.retry_max_delay), client);

        let client = Timeout::new(client, self.timeout);
        let client = ErrorService::new(client);

        Ok(TonClient { client } )
    }
}

impl IntoFuture for TonClientBuilder {
    type Output = anyhow::Result<TonClient>;
    type IntoFuture = BoxFuture<'static, Self::Output>;

    fn into_future(self) -> Self::IntoFuture {
        self.build().boxed()
    }
}

impl TonClient {
    pub async fn ready(&mut self) -> anyhow::Result<()> {
        self.get_masterchain_info().await?;
        tracing::info!("ready");

        Ok(())
    }

    pub async fn from_env() -> anyhow::Result<Self> {
        let config = AppConfig::from_env()?;

        tracing::warn!("Ton config url: {}", config.config_url);
        tracing::warn!("Ton config fallback path: {:?}", config.config_path);

        TonClientBuilder::from_config_url_with_fallback(
            config.config_url,
            Duration::from_secs(60),
            config.config_path
        ).await
    }

    pub async fn get_masterchain_info(&self) -> anyhow::Result<MasterchainInfo> {
        self.client
            .clone()
            .oneshot(Specialized::new(GetMasterchainInfo::default()))
            .await
    }

    #[instrument(skip_all, err)]
    pub async fn look_up_block_by_seqno(
        &self,
        chain: i32,
        shard: i64,
        seqno: i32,
    ) -> anyhow::Result<BlockIdExt> {
        if seqno <= 0 {
            return Err(anyhow!("seqno must be greater than 0"));
        }

        self.client
            .clone()
            .oneshot(Specialized::new(BlocksLookupBlock::seqno(BlockId::new(chain, shard, seqno))))
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

        self.client
            .clone()
            .oneshot(Specialized::new(BlocksLookupBlock::logical_time(BlockId::new(chain, shard, 0), lt)))
            .await
    }

    pub async fn get_shards(&self, master_seqno: i32) -> anyhow::Result<BlocksShards> {
        let block = self
            .look_up_block_by_seqno(MAIN_CHAIN, MAIN_SHARD, master_seqno)
            .await?;

        self.client
            .clone()
            .oneshot(Specialized::new(BlocksGetShards::new(block)))
            .await
    }

    pub async fn get_shards_by_block_id(&self, block_id: BlockIdExt) -> anyhow::Result<Vec<BlockIdExt>> {
        if block_id.workchain != -1 {
            return Err(anyhow!("workchain must be -1"))
        }

        self.client
            .clone()
            .oneshot(Specialized::new(BlocksGetShards::new(block_id)))
            .map_ok(|res| res.shards)
            .await
    }

    pub async fn get_block_header(
        &self,
        chain: i32,
        shard: i64,
        seqno: i32,
    ) -> anyhow::Result<BlockHeader> {
        let id = self.look_up_block_by_seqno(chain, shard, seqno).await?;

        self.client
            .clone()
            .oneshot(BlocksGetBlockHeader::new(id))
            .await
    }

    #[instrument(skip_all, err)]
    pub async fn raw_get_account_state(&self, address: &str) -> anyhow::Result<RawFullAccountState> {
        let account_address = AccountAddress::new(address)?;

        self.client
            .clone()
            .oneshot(RawGetAccountState::new(account_address))
            .await
    }

    #[instrument(skip_all, err)]
    pub async fn raw_get_account_state_on_block(&self, address: &str, block_id: BlockIdExt) -> anyhow::Result<RawFullAccountState> {
        let account_address = AccountAddress::new(address)?;

        self.client
            .clone()
            .oneshot(WithBlock::new(block_id, RawGetAccountState::new(account_address)))
            .await
    }

    // TODO[akostylev0]: (optimization) use BlockId instead of BlockIdExt
    pub async fn raw_get_account_state_at_least_block(&self, address: &str, block_id: &BlockIdExt) -> anyhow::Result<RawFullAccountState> {
        let route = Route::Block { chain: block_id.workchain, criteria: BlockCriteria::Seqno { shard: block_id.shard, seqno: block_id.seqno } };
        let account_address = AccountAddress::new(address)?;

        self.client
            .clone()
            .oneshot(Forward::new(route, RawGetAccountState::new(account_address)))
            .await
    }

    #[instrument(skip_all, err)]
    pub async fn raw_get_account_state_by_transaction(&self, address: &str, transaction_id: InternalTransactionId) -> anyhow::Result<RawFullAccountState> {
        let account_address = AccountAddress::new(address)?;

        self.client
            .clone()
            .oneshot(RawGetAccountStateByTransaction::new(account_address, transaction_id))
            .await
    }

    pub async fn get_account_state(&self, address: &str) -> anyhow::Result<Value> {
        let account_address = AccountAddress::new(address)?;

        self.client
            .clone()
            .oneshot(GetAccountState::new(account_address))
            .await
    }

    #[instrument(skip_all, err)]
    pub async fn raw_get_transactions(
        &self,
        address: &str,
        from_tx: &InternalTransactionId
    ) -> anyhow::Result<RawTransactions> {
        let address = AccountAddress::new(address)?;

        self.client
            .clone()
            .oneshot(RawGetTransactionsV2::new(address, from_tx.clone()))
            .await
    }

    pub async fn blocks_get_transactions(
        &self,
        block: &BlockIdExt,
        tx: Option<AccountTransactionId>,
        reverse: bool,
        count: i32
    ) -> anyhow::Result<BlocksTransactions> {
        self.client
            .clone()
            .oneshot(BlocksGetTransactions::unverified(
                block.to_owned(),
                tx,
                reverse,
                count
            ))
            .await
    }

    pub async fn blocks_get_transactions_verified(
        &self,
        block: &BlockIdExt,
        tx: Option<AccountTransactionId>,
        reverse: bool,
        count: i32
    ) -> anyhow::Result<BlocksTransactions> {
        self.client
            .clone()
            .oneshot(BlocksGetTransactions::verified(
                block.to_owned(),
                tx,
                reverse,
                count
            ))
            .await
    }

    pub async fn send_message(&self, message: &str) -> anyhow::Result<Value> {
        self.client
            .clone()
            .oneshot(RawSendMessage::new(message.to_string()))
            .await
    }

    pub async fn send_message_returning_hash(&self, message: &str) -> anyhow::Result<String> {
        self.client
            .clone()
            .oneshot(RawSendMessageReturnHash::new(message.to_string()))
            .map_ok(|res| res.hash)
            .await
    }

    pub fn get_block_tx_stream_unordered(&self, block: &BlockIdExt) -> impl Stream<Item=anyhow::Result<ShortTxId>> + 'static {
        let streams = Side::values().map(move |side| {
            (side, self.get_block_tx_stream(block, side.is_right()).boxed())
        });
        let stream_map = StreamMap::from_iter(streams);

        async_stream::try_stream! {
            let mut last = HashMap::with_capacity(2);

            for await (key, tx) in stream_map {
                let tx = tx?;
                if let Some(prev_tx) = last.get(&key.opposite()) {
                    if prev_tx == &tx {
                        return;
                    }
                }
                last.insert(key, tx.clone());
                yield tx;
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
    #[instrument(skip_all, err)]
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

    #[instrument(skip_all)]
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
        try_stream! {
            tokio::pin!(stream);
            while let Some(x) = stream.try_next().await? {
                match end.as_ref() {
                    Bound::Unbounded => { yield x; },
                    Bound::Included(tx) => {
                        let cond = tx == &x.transaction_id ;
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

    #[instrument(skip_all)]
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
                let Some(tx_id) = state.last_transaction_id else {
                    return anyhow::Ok(None);
                };

                tx_id
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

        self.client
            .clone()
            .oneshot(RunGetMethod::new(address, method, stack))
            .await
    }

    pub async fn get_shard_account_cell(&self, address: &str) -> anyhow::Result<Cell> {
        let address = AccountAddress::new(address)?;

        self.client
            .clone()
            .oneshot(GetShardAccountCell::new(address))
            .await
    }

    pub async fn get_shard_account_cell_on_block(&self, address: &str, block: BlockIdExt) -> anyhow::Result<Cell> {
        let address = AccountAddress::new(address)?;

        self.client
            .clone()
            .oneshot(WithBlock::new(block, GetShardAccountCell::new(address)))
            .await
    }

    // TODO[akostylev0]: (optimization) use BlockId instead of BlockIdExt
    pub async fn get_shard_account_cell_at_least_block(&self, address: &str, block_id: &BlockIdExt) -> anyhow::Result<Cell> {
        let route = Route::Block { chain: block_id.workchain, criteria: BlockCriteria::Seqno { shard: block_id.shard, seqno: block_id.seqno } };
        let address = AccountAddress::new(address)?;

        self.client
            .clone()
            .oneshot(Forward::new(route, GetShardAccountCell::new(address)))
            .await
    }

    pub async fn get_shard_account_cell_by_transaction(&self, address: &str, transaction: InternalTransactionId) -> anyhow::Result<Cell> {
        let address = AccountAddress::new(address)?;

        self.client
            .clone()
            .oneshot(GetShardAccountCellByTransaction::new(address, transaction))
            .await
    }

    pub fn get_accounts_in_block_stream(&self, block: &BlockIdExt) -> impl TryStream<Ok=InternalAccountAddress, Error=anyhow::Error> + 'static {
        let chain = block.workchain;
        let streams = Side::values().map(move |side| {
            (side, self.get_block_tx_stream(block, side.is_right()).boxed())
        });
        let stream_map = StreamMap::from_iter(streams);

        let stream = async_stream::try_stream! {
            let mut last = HashMap::with_capacity(2);

            for await (key, tx) in stream_map {
                let tx = tx?;

                if let Some(addr) = last.get(&key.opposite()) {
                    if addr == &tx.account { return }
                }

                if let Some(addr) = last.get(&key) {
                    if addr == &tx.account { continue }
                }

                last.insert(key, tx.account.clone());
                yield tx.account;
            }
        };

        stream.map_ok(move |a: ShardContextAccountAddress| a.into_internal(chain))
    }

    #[instrument(skip_all, err)]
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
