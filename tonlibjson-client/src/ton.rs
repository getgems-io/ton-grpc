use std::cmp::min;
use std::collections::{Bound, HashMap};
use std::ops::{RangeBounds};
use std::path::PathBuf;
use std::pin::Pin;
use std::time::Duration;
use futures::{Stream, stream, TryStreamExt, StreamExt, try_join, TryStream, TryFutureExt};
use anyhow::anyhow;
use async_stream::try_stream;
use itertools::Itertools;
use serde_json::Value;
use tokio_stream::StreamMap;
use tower::load::PeakEwmaDiscover;
use tower::retry::budget::Budget;
use tower::retry::Retry;
use tower::{Layer, ServiceExt};
use tower::timeout::Timeout;
use tracing::{instrument, trace};
use url::Url;
use std::str::FromStr;
use tokio::time::MissedTickBehavior;
use tokio_util::either;
use tower::discover::Change;
use tower::util::Either;
use ton_client_util::discover::{LiteServerDiscover, read_ton_config_from_file_stream, read_ton_config_from_url_stream};
use ton_client_util::discover::config::LiteServerId;
use ton_client_util::router::route::{BlockCriteria, Route};
use ton_client_util::router::balance::Balance;
use crate::address::InternalAccountAddress;
use crate::block::{InternalTransactionId, RawTransaction, RawTransactions, BlocksShards, BlocksTransactions, RawSendMessage, AccountAddress, BlocksGetTransactions, BlocksLookupBlock, BlocksGetShards, BlocksGetBlockHeader, RawGetTransactionsV2, RawGetAccountState, GetAccountState, GetShardAccountCell, RawFullAccountState, WithBlock, RawGetAccountStateByTransaction, GetShardAccountCellByTransaction, RawSendMessageReturnHash, BlocksMasterchainInfo, BlocksGetMasterchainInfo, TonBlockIdExt, TonBlockId, BlocksHeader, FullAccountState, BlocksAccountTransactionId, BlocksShortTxId, TvmBoxedStackEntry, SmcRunResult, SmcBoxedMethodId, TvmCell, BlocksGetTransactionsExt, BlocksTransactionsExt};
use crate::error::ErrorService;
use crate::request::{Forward, Specialized};
use crate::retry::RetryPolicy;
use crate::session::RunGetMethod;
use ton_client_util::service::shared::SharedService;
use crate::cursor_client::CursorClient;
use crate::make::{ClientFactory, CursorClientFactory};

#[cfg(not(feature = "testnet"))]
pub fn default_ton_config_url() -> Url {
    Url::from_str("https://raw.githubusercontent.com/ton-blockchain/ton-blockchain.github.io/main/global.config.json").unwrap()
}

#[cfg(feature = "testnet")]
pub fn default_ton_config_url() -> Url {
    Url::from_str("https://raw.githubusercontent.com/ton-blockchain/ton-blockchain.github.io/main/testnet-global.config.json").unwrap()
}

type BoxCursorClientDiscover = Pin<Box<dyn Stream<Item=Result<Change<LiteServerId, CursorClient>, anyhow::Error>> + Send>>;
type SharedBalance = SharedService<Balance<CursorClient, BoxCursorClientDiscover>>;

#[derive(Clone)]
pub struct TonClient {
    client: ErrorService<Timeout<Either<Retry<RetryPolicy, SharedBalance>, SharedBalance>>>
}

const MAIN_CHAIN: i32 = -1;
const MAIN_SHARD: i64 = -9223372036854775808;

enum ConfigSource {
    FromFile { path: PathBuf },
    FromUrl { url: Url, interval: Duration }
}

pub struct TonClientBuilder {
    config_source: ConfigSource,
    timeout: Duration,
    ewma_default_rtt: Duration,
    ewma_decay: Duration,
    retry_enabled: bool,
    retry_budget_ttl: Duration,
    retry_min_per_sec: u32,
    retry_percent: f32,
    retry_first_delay: Duration,
    retry_max_delay: Duration
}

impl Default for TonClientBuilder {
    fn default() -> Self {
        Self {
            config_source: ConfigSource::FromUrl { url: default_ton_config_url(), interval: Duration::from_secs(60) },
            timeout: Duration::from_secs(10),
            ewma_default_rtt: Duration::from_millis(70),
            ewma_decay: Duration::from_millis(1),
            retry_enabled: true,
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
            config_source: ConfigSource::FromUrl { url, interval },
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

    pub fn disable_retry(mut self) -> Self {
        self.retry_enabled = false;

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

    pub fn build(self) -> anyhow::Result<TonClient> {
        let stream = match self.config_source {
            ConfigSource::FromFile { path } => {
                let mut interval = tokio::time::interval(Duration::from_secs(1));
                interval.set_missed_tick_behavior(MissedTickBehavior::Skip);

                either::Either::Left(read_ton_config_from_file_stream(path, interval))
            }
            ConfigSource::FromUrl { url, interval } => {
                let mut interval = tokio::time::interval(interval);
                interval.set_missed_tick_behavior(MissedTickBehavior::Skip);

                either::Either::Right(read_ton_config_from_url_stream(url.clone(), interval))
            }
        };
        let lite_server_discover = LiteServerDiscover::new(stream);
        let client_discover = lite_server_discover.then(|s| async {
            match s {
                Ok(Change::Insert(k, v)) => ClientFactory.oneshot(v).await.map(|v| Change::Insert(k, v)),
                Ok(Change::Remove(k)) => Ok(Change::Remove(k)),
                Err(_) => unreachable!()
            }
        });

        let ewma_discover = PeakEwmaDiscover::new::<Value>(
            client_discover,
            self.ewma_default_rtt,
            self.ewma_decay,
            tower::load::CompleteOnResponse::default(),
        );

        let cursor_client_discover = ewma_discover.then(|s| async {
            match s {
                Ok(Change::Insert(k, v)) => Ok(Change::Insert(k.clone(), CursorClientFactory::create(k, v))),
                Ok(Change::Remove(k)) => Ok(Change::Remove(k)),
                Err(e) => Err(e)
            }
        });

        let client = Balance::new(cursor_client_discover.boxed());

        let client = SharedService::new(client);
        let client = tower::util::option_layer(if self.retry_enabled {
            Some(tower::retry::RetryLayer::new(RetryPolicy::new(Budget::new(
                self.retry_budget_ttl,
                self.retry_min_per_sec,
                self.retry_percent
            ), self.retry_first_delay.as_millis() as u64, self.retry_max_delay)))
        } else { None }).layer(client);

        let client = Timeout::new(client, self.timeout);
        let client = ErrorService::new(client);

        Ok(TonClient { client } )
    }
}

impl TonClient {
    pub async fn ready(&mut self) -> anyhow::Result<()> {
        self.get_masterchain_info().await?;
        tracing::info!("ready");

        Ok(())
    }

    pub async fn get_masterchain_info(&self) -> anyhow::Result<BlocksMasterchainInfo> {
        self.client
            .clone()
            .oneshot(Specialized::new(BlocksGetMasterchainInfo::default()))
            .await
    }

    #[instrument(skip_all, err)]
    pub async fn look_up_block_by_seqno(
        &self,
        chain: i32,
        shard: i64,
        seqno: i32,
    ) -> anyhow::Result<TonBlockIdExt> {
        if seqno <= 0 {
            return Err(anyhow!("seqno must be greater than 0"));
        }

        self.client
            .clone()
            .oneshot(BlocksLookupBlock::seqno(TonBlockId::new(chain, shard, seqno)))
            .await
    }

    pub async fn look_up_block_by_lt(
        &self,
        chain: i32,
        shard: i64,
        lt: i64,
    ) -> anyhow::Result<TonBlockIdExt> {
        if lt <= 0 {
            return Err(anyhow!("lt must be greater than 0"));
        }

        self.client
            .clone()
            .oneshot(BlocksLookupBlock::logical_time(TonBlockId::new(chain, shard, 0), lt))
            .await
    }

    pub async fn get_shards(&self, master_seqno: i32) -> anyhow::Result<BlocksShards> {
        let block = self
            .look_up_block_by_seqno(MAIN_CHAIN, MAIN_SHARD, master_seqno)
            .await?;

        self.client
            .clone()
            .oneshot(BlocksGetShards::new(block))
            .await
    }

    pub async fn get_shards_by_block_id(&self, block_id: TonBlockIdExt) -> anyhow::Result<Vec<TonBlockIdExt>> {
        if block_id.workchain != -1 {
            return Err(anyhow!("workchain must be -1"))
        }

        self.client
            .clone()
            .oneshot(BlocksGetShards::new(block_id))
            .map_ok(|res| res.shards)
            .await
    }

    pub async fn get_block_header(
        &self,
        workchain: i32,
        shard: i64,
        seqno: i32,
        hashes: Option<(String, String)>,
    ) -> anyhow::Result<BlocksHeader> {
        let (root_hash, file_hash) = match hashes {
            Some((root_hash,file_hash)) => (root_hash, file_hash),
            _ => {
                let block = self.look_up_block_by_seqno(workchain, shard, seqno).await?;
                (block.root_hash, block.file_hash)
            }
        };

        self.client
            .clone()
            .oneshot(BlocksGetBlockHeader::new(TonBlockIdExt {
                workchain,
                shard,
                seqno,
                root_hash,
                file_hash,
            }))
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
    pub async fn raw_get_account_state_on_block(&self, address: &str, block_id: TonBlockIdExt) -> anyhow::Result<RawFullAccountState> {
        let account_address = AccountAddress::new(address)?;

        self.client
            .clone()
            .oneshot(WithBlock::new(block_id, RawGetAccountState::new(account_address)))
            .await
    }

    // TODO[akostylev0]: (optimization) use BlockId instead of BlockIdExt
    pub async fn raw_get_account_state_at_least_block(&self, address: &str, block_id: &TonBlockIdExt) -> anyhow::Result<RawFullAccountState> {
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

    pub async fn get_account_state(&self, address: &str) -> anyhow::Result<FullAccountState> {
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
            .oneshot(RawGetTransactionsV2::new(address, from_tx.clone(), 16, false))
            .await
    }

    pub async fn blocks_get_transactions_ext(
        &self,
        block: &TonBlockIdExt,
        tx: Option<BlocksAccountTransactionId>,
        reverse: bool,
        count: i32
    ) -> anyhow::Result<BlocksTransactionsExt> {
        self.client
            .clone()
            .oneshot(BlocksGetTransactionsExt::unverified(
                block.to_owned(),
                tx,
                reverse,
                count
            ))
            .await
    }

    pub async fn blocks_get_transactions(
        &self,
        block: &TonBlockIdExt,
        tx: Option<BlocksAccountTransactionId>,
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
        block: &TonBlockIdExt,
        tx: Option<BlocksAccountTransactionId>,
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

    pub async fn send_message(&self, message: &str) -> anyhow::Result<()> {
        self.client
            .clone()
            .oneshot(RawSendMessage::new(message.to_string()))
            .await?;

        Ok(())
    }

    pub async fn send_message_returning_hash(&self, message: &str) -> anyhow::Result<String> {
        self.client
            .clone()
            .oneshot(RawSendMessageReturnHash::new(message.to_string()))
            .map_ok(|res| res.hash)
            .await
    }

    pub fn get_block_tx_stream_unordered(&self, block: &TonBlockIdExt) -> impl Stream<Item=anyhow::Result<BlocksShortTxId>> + 'static {
        let stream_map = StreamMap::from_iter([false, true]
            .map(|r| (r, self.get_block_tx_id_stream(block, r).boxed())));

        try_stream! {
            let mut last = HashMap::with_capacity(2);

            for await (key, tx) in stream_map {
                let tx = tx?;
                if let Some(prev_tx) = last.get(&!key) {
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
        block: &TonBlockIdExt,
        reverse: bool
    ) -> impl Stream<Item=anyhow::Result<RawTransaction>> + 'static {
        struct State {
            last_tx: Option<BlocksAccountTransactionId>,
            incomplete: bool,
            block: TonBlockIdExt,
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

                    let txs = state.this.blocks_get_transactions_ext(&state.block, state.last_tx, reverse, 2_i32.pow(state.exp)).await?;

                    tracing::debug!("got {} transactions", txs.transactions.len());

                    let last_tx = txs.transactions.last()
                        .map(|t| t.try_into()).transpose()?;

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

    pub fn get_block_tx_id_stream(
        &self,
        block: &TonBlockIdExt,
        reverse: bool
    ) -> impl Stream<Item=anyhow::Result<BlocksShortTxId>> + 'static {
        struct State {
            last_tx: Option<BlocksAccountTransactionId>,
            incomplete: bool,
            block: TonBlockIdExt,
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

    pub async fn run_get_method(&self, address: String, method: String, stack: Vec<TvmBoxedStackEntry>) -> anyhow::Result<SmcRunResult> {
        let address = AccountAddress::new(&address)?;
        let method = SmcBoxedMethodId::by_name(&method);

        self.client
            .clone()
            .oneshot(RunGetMethod::new(address, method, stack))
            .await
    }

    pub async fn get_shard_account_cell(&self, address: &str) -> anyhow::Result<TvmCell> {
        let address = AccountAddress::new(address)?;

        self.client
            .clone()
            .oneshot(GetShardAccountCell::new(address))
            .await
    }

    pub async fn get_shard_account_cell_on_block(&self, address: &str, block: TonBlockIdExt) -> anyhow::Result<TvmCell> {
        let address = AccountAddress::new(address)?;

        self.client
            .clone()
            .oneshot(WithBlock::new(block, GetShardAccountCell::new(address)))
            .await
    }

    // TODO[akostylev0]: (optimization) use BlockId instead of BlockIdExt
    pub async fn get_shard_account_cell_at_least_block(&self, address: &str, block_id: &TonBlockIdExt) -> anyhow::Result<TvmCell> {
        let route = Route::Block { chain: block_id.workchain, criteria: BlockCriteria::Seqno { shard: block_id.shard, seqno: block_id.seqno } };
        let address = AccountAddress::new(address)?;

        self.client
            .clone()
            .oneshot(Forward::new(route, GetShardAccountCell::new(address)))
            .await
    }

    pub async fn get_shard_account_cell_by_transaction(&self, address: &str, transaction: InternalTransactionId) -> anyhow::Result<TvmCell> {
        let address = AccountAddress::new(address)?;

        self.client
            .clone()
            .oneshot(GetShardAccountCellByTransaction::new(address, transaction))
            .await
    }

    pub fn get_accounts_in_block_stream(&self, block: &TonBlockIdExt) -> impl TryStream<Ok=InternalAccountAddress, Error=anyhow::Error> + 'static {
        let chain = block.workchain;
        let stream_map = StreamMap::from_iter([false, true]
            .map(|r| (r, self.get_block_tx_id_stream(block, r).boxed())));

        let stream = try_stream! {
            let mut last = HashMap::with_capacity(2);

            for await (key, tx) in stream_map {
                let tx = tx?;

                if let Some(addr) = last.get(&!key) {
                    if addr == tx.account() { return }
                }

                if let Some(addr) = last.get(&key) {
                    if addr == tx.account() { continue }
                }

                last.insert(key, tx.account().to_owned());

                yield tx.into_internal(chain);
            }
        };

        stream
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

        let mut tx = self.check_account_available(account, &TonBlockId::new(workchain, shard, cur)).await;

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

            tx = self.check_account_available(account, &TonBlockId::new(workchain, shard, cur)).await;
        }

        let tx = tx?;

        trace!(tx = ?tx, "first tx");

        Ok(tx)
    }

    async fn check_account_available(&self, account: &str, block: &TonBlockId) -> anyhow::Result<InternalTransactionId> {
        let block = self
            .look_up_block_by_seqno(block.workchain, block.shard, block.seqno).await?;
        let state = self
            .raw_get_account_state_on_block(account, block).await?;

        state.last_transaction_id.ok_or(anyhow!("tx not found"))
    }
}
