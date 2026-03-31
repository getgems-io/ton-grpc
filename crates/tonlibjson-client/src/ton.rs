use crate::block::{
    AccountAddress, BlocksAccountTransactionId, BlocksGetBlockHeader, BlocksGetMasterchainInfo,
    BlocksGetShards, BlocksGetTransactions, BlocksGetTransactionsExt, BlocksHeader,
    BlocksLookupBlock, BlocksShards, BlocksTransactions, BlocksTransactionsExt,
    GetShardAccountCell, GetShardAccountCellByTransaction, InternalTransactionId,
    RawFullAccountState, RawGetAccountState, RawGetAccountStateByTransaction, RawGetTransactionsV2,
    RawSendMessage, RawSendMessageReturnHash, RawTransactions, SmcBoxedMethodId, SmcRunResult,
    TonBlockId, TonBlockIdExt, TvmBoxedStackEntry, TvmCell, WithBlock,
};
use crate::cursor::client::CursorClient;
use crate::error::ErrorService;
use crate::make::{ClientFactory, CursorClientFactory};
use crate::request::{Forward, Specialized};
use crate::retry::RetryPolicy;
use crate::session::RunGetMethod;
use anyhow::anyhow;
use futures::{Stream, StreamExt, TryFutureExt, stream};
use serde_json::Value;
use std::path::PathBuf;
use std::pin::Pin;
use std::str::FromStr;
use std::time::Duration;
use tokio::time::MissedTickBehavior;
use ton_client::TonClient as _;
use ton_client_util::discover::config::{LiteServerId, TonConfig};
use ton_client_util::discover::{
    LiteServerDiscover, read_ton_config_from_file_stream, read_ton_config_from_url_stream,
};
use ton_client_util::router::balance::Balance;
use ton_client_util::router::route::{BlockCriteria, Route};
use ton_client_util::service::shared::SharedService;
use tower::discover::Change;
use tower::load::PeakEwmaDiscover;
use tower::retry::Retry;
use tower::retry::budget::TpsBudget;
use tower::timeout::Timeout;
use tower::util::Either;
use tower::{Layer, ServiceExt};
use tracing::instrument;
use url::Url;

#[cfg(not(feature = "testnet"))]
pub fn default_ton_config_url() -> Url {
    Url::from_str("https://raw.githubusercontent.com/ton-blockchain/ton-blockchain.github.io/main/global.config.json").unwrap()
}

#[cfg(feature = "testnet")]
pub fn default_ton_config_url() -> Url {
    Url::from_str("https://raw.githubusercontent.com/ton-blockchain/ton-blockchain.github.io/main/testnet-global.config.json").unwrap()
}

type BoxCursorClientDiscover =
    Pin<Box<dyn Stream<Item = Result<Change<LiteServerId, CursorClient>, anyhow::Error>> + Send>>;
type SharedBalance = SharedService<Balance<CursorClient, BoxCursorClientDiscover>>;

#[derive(Clone)]
pub struct TonClient {
    client: ErrorService<Timeout<Either<Retry<RetryPolicy, SharedBalance>, SharedBalance>>>,
}

const MAIN_CHAIN: i32 = -1;
const MAIN_SHARD: i64 = -9223372036854775808;

enum ConfigSource {
    File { path: PathBuf },
    Url { url: Url, interval: Duration },
    Config { config: String },
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
    retry_max_delay: Duration,
}

impl Default for TonClientBuilder {
    fn default() -> Self {
        Self {
            config_source: ConfigSource::Url {
                url: default_ton_config_url(),
                interval: Duration::from_secs(60),
            },
            timeout: Duration::from_secs(10),
            ewma_default_rtt: Duration::from_millis(70),
            ewma_decay: Duration::from_millis(1),
            retry_enabled: true,
            retry_budget_ttl: Duration::from_secs(10),
            retry_min_per_sec: 10,
            retry_percent: 0.1,
            retry_first_delay: Duration::from_millis(128),
            retry_max_delay: Duration::from_millis(4096),
        }
    }
}

impl TonClientBuilder {
    pub fn from_config_path(path: PathBuf) -> Self {
        Self {
            config_source: ConfigSource::File { path },
            ..Default::default()
        }
    }

    pub fn from_config_url(url: Url, interval: Duration) -> Self {
        Self {
            config_source: ConfigSource::Url { url, interval },
            ..Default::default()
        }
    }

    pub fn from_config(config: &str) -> Self {
        Self {
            config_source: ConfigSource::Config {
                config: config.to_owned(),
            },
            ..Default::default()
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
        let stream: Pin<Box<dyn Stream<Item = Result<TonConfig, anyhow::Error>> + Send>> =
            match self.config_source {
                ConfigSource::File { path } => {
                    let mut interval = tokio::time::interval(Duration::from_secs(1));
                    interval.set_missed_tick_behavior(MissedTickBehavior::Skip);

                    Box::pin(read_ton_config_from_file_stream(path, interval))
                }
                ConfigSource::Url { url, interval } => {
                    let mut interval = tokio::time::interval(interval);
                    interval.set_missed_tick_behavior(MissedTickBehavior::Skip);

                    Box::pin(read_ton_config_from_url_stream(url, interval))
                }
                ConfigSource::Config { config } => {
                    let config: TonConfig = serde_json::from_str(&config)?;
                    Box::pin(stream::once(async { Ok(config) }))
                }
            };
        let lite_server_discover = LiteServerDiscover::new(stream);
        let client_discover = lite_server_discover.then(|s| async {
            match s {
                Ok(Change::Insert(k, v)) => {
                    ClientFactory.oneshot(v).await.map(|v| Change::Insert(k, v))
                }
                Ok(Change::Remove(k)) => Ok(Change::Remove(k)),
                Err(_) => unreachable!(),
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
                Ok(Change::Insert(k, v)) => {
                    Ok(Change::Insert(k.clone(), CursorClientFactory::create(k, v)))
                }
                Ok(Change::Remove(k)) => Ok(Change::Remove(k)),
                Err(e) => Err(e),
            }
        });

        let client = Balance::new(cursor_client_discover.boxed());

        let client = SharedService::new(client);
        let client = tower::util::option_layer(if self.retry_enabled {
            Some(tower::retry::RetryLayer::new(RetryPolicy::new(
                TpsBudget::new(
                    self.retry_budget_ttl,
                    self.retry_min_per_sec,
                    self.retry_percent,
                ),
                self.retry_first_delay.as_millis() as u64,
                self.retry_max_delay,
            )))
        } else {
            None
        })
        .layer(client);

        let client = Timeout::new(client, self.timeout);
        let client = ErrorService::new(client);

        Ok(TonClient { client })
    }
}

impl TonClient {
    pub async fn ready(&mut self) -> anyhow::Result<()> {
        self.get_masterchain_info().await?;
        tracing::info!("ready");

        Ok(())
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
            .oneshot(BlocksLookupBlock::seqno(TonBlockId::new(
                chain, shard, seqno,
            )))
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
            .oneshot(BlocksLookupBlock::logical_time(
                TonBlockId::new(chain, shard, 0),
                lt,
            ))
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

    pub async fn get_shards_by_block_id(
        &self,
        block_id: TonBlockIdExt,
    ) -> anyhow::Result<Vec<TonBlockIdExt>> {
        if block_id.workchain != -1 {
            return Err(anyhow!("workchain must be -1"));
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
            Some((root_hash, file_hash)) => (root_hash, file_hash),
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
    pub async fn raw_get_account_state(
        &self,
        address: &str,
    ) -> anyhow::Result<RawFullAccountState> {
        let account_address = AccountAddress::new(address)?;

        self.client
            .clone()
            .oneshot(RawGetAccountState::new(account_address))
            .await
    }

    #[instrument(skip_all, err)]
    pub async fn raw_get_account_state_on_block(
        &self,
        address: &str,
        block_id: TonBlockIdExt,
    ) -> anyhow::Result<RawFullAccountState> {
        let account_address = AccountAddress::new(address)?;

        self.client
            .clone()
            .oneshot(WithBlock::new(
                block_id,
                RawGetAccountState::new(account_address),
            ))
            .await
    }

    // TODO[akostylev0]: (optimization) use BlockId instead of BlockIdExt
    pub async fn raw_get_account_state_at_least_block(
        &self,
        address: &str,
        block_id: &TonBlockIdExt,
    ) -> anyhow::Result<RawFullAccountState> {
        let route = Route::Block {
            chain: block_id.workchain,
            criteria: BlockCriteria::Seqno {
                shard: block_id.shard,
                seqno: block_id.seqno,
            },
        };
        let account_address = AccountAddress::new(address)?;

        self.client
            .clone()
            .oneshot(Forward::new(
                route,
                RawGetAccountState::new(account_address),
            ))
            .await
    }

    #[instrument(skip_all, err)]
    pub async fn raw_get_account_state_by_transaction(
        &self,
        address: &str,
        transaction_id: InternalTransactionId,
    ) -> anyhow::Result<RawFullAccountState> {
        let account_address = AccountAddress::new(address)?;

        self.client
            .clone()
            .oneshot(RawGetAccountStateByTransaction::new(
                account_address,
                transaction_id,
            ))
            .await
    }

    #[instrument(skip_all, err)]
    pub async fn raw_get_transactions(
        &self,
        address: &str,
        from_tx: &InternalTransactionId,
    ) -> anyhow::Result<RawTransactions> {
        let address = AccountAddress::new(address)?;

        self.client
            .clone()
            .oneshot(RawGetTransactionsV2::new(
                address,
                from_tx.clone(),
                16,
                false,
            ))
            .await
    }

    pub async fn blocks_get_transactions_ext(
        &self,
        block: &TonBlockIdExt,
        tx: Option<BlocksAccountTransactionId>,
        reverse: bool,
        count: i32,
    ) -> anyhow::Result<BlocksTransactionsExt> {
        self.client
            .clone()
            .oneshot(BlocksGetTransactionsExt::unverified(
                block.to_owned(),
                tx,
                reverse,
                count,
            ))
            .await
    }

    pub async fn blocks_get_transactions(
        &self,
        block: &TonBlockIdExt,
        tx: Option<BlocksAccountTransactionId>,
        reverse: bool,
        count: i32,
    ) -> anyhow::Result<BlocksTransactions> {
        self.client
            .clone()
            .oneshot(BlocksGetTransactions::unverified(
                block.to_owned(),
                tx,
                reverse,
                count,
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

    pub async fn run_get_method(
        &self,
        address: String,
        method: String,
        stack: Vec<TvmBoxedStackEntry>,
    ) -> anyhow::Result<SmcRunResult> {
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

    pub async fn get_shard_account_cell_on_block(
        &self,
        address: &str,
        block: TonBlockIdExt,
    ) -> anyhow::Result<TvmCell> {
        let address = AccountAddress::new(address)?;

        self.client
            .clone()
            .oneshot(WithBlock::new(block, GetShardAccountCell::new(address)))
            .await
    }

    // TODO[akostylev0]: (optimization) use BlockId instead of BlockIdExt
    pub async fn get_shard_account_cell_at_least_block(
        &self,
        address: &str,
        block_id: &TonBlockIdExt,
    ) -> anyhow::Result<TvmCell> {
        let route = Route::Block {
            chain: block_id.workchain,
            criteria: BlockCriteria::Seqno {
                shard: block_id.shard,
                seqno: block_id.seqno,
            },
        };
        let address = AccountAddress::new(address)?;

        self.client
            .clone()
            .oneshot(Forward::new(route, GetShardAccountCell::new(address)))
            .await
    }

    pub async fn get_shard_account_cell_by_transaction(
        &self,
        address: &str,
        transaction: InternalTransactionId,
    ) -> anyhow::Result<TvmCell> {
        let address = AccountAddress::new(address)?;

        self.client
            .clone()
            .oneshot(GetShardAccountCellByTransaction::new(address, transaction))
            .await
    }
}

#[async_trait::async_trait]
impl ton_client::TonClient for TonClient {
    async fn get_masterchain_info(&self) -> anyhow::Result<ton_client::MasterchainInfo> {
        self.client
            .clone()
            .oneshot(Specialized::new(BlocksGetMasterchainInfo::default()))
            .await
            .map(Into::into)
    }

    async fn look_up_block_by_seqno(
        &self,
        chain: i32,
        shard: i64,
        seqno: i32,
    ) -> anyhow::Result<ton_client::BlockIdExt> {
        self.look_up_block_by_seqno(chain, shard, seqno)
            .await
            .map(Into::into)
    }

    async fn look_up_block_by_lt(
        &self,
        chain: i32,
        shard: i64,
        lt: i64,
    ) -> anyhow::Result<ton_client::BlockIdExt> {
        self.look_up_block_by_lt(chain, shard, lt)
            .await
            .map(Into::into)
    }

    async fn get_shards(&self, master_seqno: i32) -> anyhow::Result<ton_client::Shards> {
        self.get_shards(master_seqno).await.map(Into::into)
    }

    async fn get_block_header(
        &self,
        id: ton_client::BlockIdExt,
    ) -> anyhow::Result<ton_client::BlockHeader> {
        self.get_block_header(
            id.workchain,
            id.shard,
            id.seqno,
            Some((id.root_hash, id.file_hash)),
        )
        .await
        .map(Into::into)
    }

    async fn get_account_state(&self, address: &str) -> anyhow::Result<ton_client::AccountState> {
        self.raw_get_account_state(address).await.map(Into::into)
    }

    async fn get_account_state_on_block(
        &self,
        address: &str,
        block_id: ton_client::BlockIdExt,
    ) -> anyhow::Result<ton_client::AccountState> {
        self.raw_get_account_state_on_block(address, block_id.into())
            .await
            .map(Into::into)
    }

    async fn get_account_state_by_transaction(
        &self,
        address: &str,
        tx: ton_client::TransactionId,
    ) -> anyhow::Result<ton_client::AccountState> {
        self.raw_get_account_state_by_transaction(address, tx.into())
            .await
            .map(Into::into)
    }

    async fn get_transactions(
        &self,
        address: &str,
        from: &ton_client::TransactionId,
    ) -> anyhow::Result<ton_client::Transactions> {
        let from = InternalTransactionId {
            lt: from.lt,
            hash: from.hash.clone(),
        };
        self.raw_get_transactions(address, &from)
            .await
            .map(Into::into)
    }

    async fn get_shard_account_cell(&self, address: &str) -> anyhow::Result<ton_client::Cell> {
        self.get_shard_account_cell(address).await.map(Into::into)
    }

    async fn get_shard_account_cell_on_block(
        &self,
        address: &str,
        block: ton_client::BlockIdExt,
    ) -> anyhow::Result<ton_client::Cell> {
        self.get_shard_account_cell_on_block(address, block.into())
            .await
            .map(Into::into)
    }

    async fn get_shard_account_cell_by_transaction(
        &self,
        address: &str,
        tx: ton_client::TransactionId,
    ) -> anyhow::Result<ton_client::Cell> {
        self.get_shard_account_cell_by_transaction(address, tx.into())
            .await
            .map(Into::into)
    }

    async fn run_get_method(
        &self,
        address: &str,
        method: &str,
        stack: Vec<ton_client::StackEntry>,
    ) -> anyhow::Result<ton_client::SmcRunResult> {
        self.run_get_method(
            address.to_string(),
            method.to_string(),
            stack.into_iter().map(Into::into).collect(),
        )
        .await
        .map(Into::into)
    }

    async fn send_message(&self, message: &str) -> anyhow::Result<()> {
        self.send_message(message).await
    }

    async fn send_message_returning_hash(&self, message: &str) -> anyhow::Result<String> {
        self.send_message_returning_hash(message).await
    }

    async fn get_shards_by_block_id(
        &self,
        block_id: ton_client::BlockIdExt,
    ) -> anyhow::Result<Vec<ton_client::BlockIdExt>> {
        self.get_shards_by_block_id(block_id.into())
            .await
            .map(|v| v.into_iter().map(Into::into).collect())
    }

    async fn get_account_state_at_least_block(
        &self,
        address: &str,
        block_id: &ton_client::BlockIdExt,
    ) -> anyhow::Result<ton_client::AccountState> {
        let block_id: TonBlockIdExt = ton_client::BlockIdExt::clone(block_id).into();
        self.raw_get_account_state_at_least_block(address, &block_id)
            .await
            .map(Into::into)
    }

    async fn get_shard_account_cell_at_least_block(
        &self,
        address: &str,
        block_id: &ton_client::BlockIdExt,
    ) -> anyhow::Result<ton_client::Cell> {
        let block_id: TonBlockIdExt = ton_client::BlockIdExt::clone(block_id).into();
        self.get_shard_account_cell_at_least_block(address, &block_id)
            .await
            .map(Into::into)
    }

    async fn blocks_get_transactions(
        &self,
        block: &ton_client::BlockIdExt,
        after: Option<ton_client::ShortTxId>,
        reverse: bool,
        count: i32,
    ) -> anyhow::Result<ton_client::BlockTransactions> {
        let block: TonBlockIdExt = block.clone().into();
        let after: Option<BlocksAccountTransactionId> = after.map(Into::into);
        self.blocks_get_transactions(&block, after, reverse, count)
            .await
            .map(Into::into)
    }

    async fn blocks_get_transactions_ext(
        &self,
        block: &ton_client::BlockIdExt,
        after: Option<ton_client::ShortTxId>,
        reverse: bool,
        count: i32,
    ) -> anyhow::Result<ton_client::BlockTransactionsExt> {
        let block: TonBlockIdExt = block.clone().into();
        let after: Option<BlocksAccountTransactionId> = after.map(Into::into);
        self.blocks_get_transactions_ext(&block, after, reverse, count)
            .await
            .map(Into::into)
    }
}

#[cfg(test)]
mod integration {
    use super::*;
    use testcontainers_ton::LocalLiteServer;

    #[tokio::test]
    async fn should_get_masterchain_info() -> anyhow::Result<()> {
        let server = LocalLiteServer::new().await?;
        let mut client = TonClientBuilder::from_config(server.config()).build()?;
        client.ready().await?;

        let masterchain_info = client.get_masterchain_info().await?;

        assert!(masterchain_info.last.seqno > 0);

        Ok(())
    }

    #[tokio::test]
    async fn should_get_block_header() -> anyhow::Result<()> {
        let server = LocalLiteServer::new().await?;
        let mut client = TonClientBuilder::from_config(server.config()).build()?;
        client.ready().await?;

        let info = client.get_masterchain_info().await?;
        let header = client
            .get_block_header(
                info.last.workchain,
                info.last.shard,
                info.last.seqno,
                Some((info.last.root_hash, info.last.file_hash)),
            )
            .await?;

        assert_eq!(header.id.workchain, -1);

        Ok(())
    }
}
