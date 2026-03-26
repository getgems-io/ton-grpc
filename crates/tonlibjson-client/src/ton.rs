use crate::block::{
    AccountAddress, BlocksAccountTransactionId, BlocksGetBlockHeader, BlocksGetMasterchainInfo,
    BlocksGetShards, BlocksGetTransactions, BlocksGetTransactionsExt, BlocksHeader,
    BlocksLookupBlock, BlocksMasterchainInfo, BlocksShards, BlocksShortTxId, BlocksTransactions,
    BlocksTransactionsExt, GetShardAccountCell, InternalTransactionId, MsgBoxedData,
    RawFullAccountState, RawGetAccountState, RawGetAccountStateByTransaction, RawGetTransactionsV2,
    RawSendMessageReturnHash, RawTransaction, RawTransactions, SmcBoxedMethodId, SmcRunResult,
    TonBlockId, TonBlockIdExt, TvmBoxedStackEntry, TvmCell, WithBlock,
};
use crate::cursor::client::CursorClient;
use crate::error::ErrorService;
use crate::make::{ClientFactory, CursorClientFactory};
use crate::request::{Forward, Specialized};
use crate::retry::RetryPolicy;
use crate::session::RunGetMethod;
use anyhow::anyhow;
use async_stream::try_stream;
use futures::stream::BoxStream;
use futures::{Stream, StreamExt, TryFutureExt, stream};
use serde_json::Value;
use std::collections::HashMap;
use std::path::PathBuf;
use std::pin::Pin;
use std::str::FromStr;
use std::time::Duration;
use tokio::time::MissedTickBehavior;
use tokio_stream::StreamMap;
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

enum ConfigSource {
    FromFile { path: PathBuf },
    FromUrl { url: Url, interval: Duration },
    Static { config: TonConfig },
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
            config_source: ConfigSource::FromUrl {
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
            config_source: ConfigSource::FromFile { path },
            ..Default::default()
        }
    }

    pub fn from_config_url(url: Url, interval: Duration) -> Self {
        Self {
            config_source: ConfigSource::FromUrl { url, interval },
            ..Default::default()
        }
    }

    pub fn from_config(config: TonConfig) -> Self {
        Self {
            config_source: ConfigSource::Static { config },
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
                ConfigSource::FromFile { path } => {
                    let mut interval = tokio::time::interval(Duration::from_secs(1));
                    interval.set_missed_tick_behavior(MissedTickBehavior::Skip);

                    Box::pin(read_ton_config_from_file_stream(path, interval))
                }
                ConfigSource::FromUrl { url, interval } => {
                    let mut interval = tokio::time::interval(interval);
                    interval.set_missed_tick_behavior(MissedTickBehavior::Skip);

                    Box::pin(read_ton_config_from_url_stream(url.clone(), interval))
                }
                ConfigSource::Static { config } => {
                    Box::pin(stream::once(async { Ok(config) }).chain(stream::pending()))
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
}

impl From<TonBlockIdExt> for ton_client::types::BlockIdExt {
    fn from(v: TonBlockIdExt) -> Self {
        Self::new(v.workchain, v.shard, v.seqno, v.root_hash, v.file_hash)
    }
}

impl From<ton_client::types::BlockIdExt> for TonBlockIdExt {
    fn from(v: ton_client::types::BlockIdExt) -> Self {
        Self {
            workchain: v.workchain,
            shard: v.shard,
            seqno: v.seqno,
            root_hash: v.root_hash,
            file_hash: v.file_hash,
        }
    }
}

impl From<InternalTransactionId> for ton_client::types::InternalTransactionId {
    fn from(v: InternalTransactionId) -> Self {
        Self {
            hash: v.hash,
            lt: v.lt,
        }
    }
}

impl From<ton_client::types::InternalTransactionId> for InternalTransactionId {
    fn from(v: ton_client::types::InternalTransactionId) -> Self {
        Self {
            hash: v.hash,
            lt: v.lt,
        }
    }
}

impl From<BlocksMasterchainInfo> for ton_client::types::MasterchainInfo {
    fn from(v: BlocksMasterchainInfo) -> Self {
        Self {
            last: v.last.into(),
        }
    }
}

impl From<BlocksShortTxId> for ton_client::types::ShortTxId {
    fn from(v: BlocksShortTxId) -> Self {
        Self {
            account: v.account,
            lt: v.lt,
            hash: v.hash,
        }
    }
}

impl From<RawFullAccountState> for ton_client::types::RawFullAccountState {
    fn from(v: RawFullAccountState) -> Self {
        Self {
            balance: v.balance,
            code: v.code,
            data: v.data,
            frozen_hash: v.frozen_hash,
            last_transaction_id: v.last_transaction_id.map(Into::into),
            block_id: v.block_id.into(),
        }
    }
}

impl From<BlocksHeader> for ton_client::types::BlocksHeader {
    fn from(v: BlocksHeader) -> Self {
        Self {
            id: v.id.into(),
            global_id: v.global_id,
            version: v.version,
            flags: v.flags,
            after_merge: v.after_merge,
            after_split: v.after_split,
            before_split: v.before_split,
            want_merge: v.want_merge,
            want_split: v.want_split,
            validator_list_hash_short: v.validator_list_hash_short,
            catchain_seqno: v.catchain_seqno,
            min_ref_mc_seqno: v.min_ref_mc_seqno,
            is_key_block: v.is_key_block,
            prev_key_block_seqno: v.prev_key_block_seqno,
            start_lt: v.start_lt,
            end_lt: v.end_lt,
            gen_utime: v.gen_utime,
            vert_seqno: v.vert_seqno,
            prev_blocks: v.prev_blocks.into_iter().map(Into::into).collect(),
        }
    }
}

impl From<TvmCell> for ton_client::types::TvmCell {
    fn from(v: TvmCell) -> Self {
        Self { bytes: v.bytes }
    }
}

impl From<AccountAddress> for ton_client::types::AccountAddress {
    fn from(v: AccountAddress) -> Self {
        Self {
            account_address: v.account_address,
        }
    }
}

impl From<MsgBoxedData> for ton_client::types::MsgData {
    fn from(v: MsgBoxedData) -> Self {
        match v {
            MsgBoxedData::MsgDataRaw(d) => Self::Raw {
                body: d.body,
                init_state: d.init_state,
            },
            MsgBoxedData::MsgDataText(d) => Self::Text { text: d.text },
            MsgBoxedData::MsgDataDecryptedText(d) => Self::DecryptedText { text: d.text },
            MsgBoxedData::MsgDataEncryptedText(d) => Self::EncryptedText { text: d.text },
        }
    }
}

impl From<crate::block::RawMessage> for ton_client::types::RawMessage {
    fn from(v: crate::block::RawMessage) -> Self {
        Self {
            source: v.source.into(),
            destination: v.destination.into(),
            value: v.value,
            fwd_fee: v.fwd_fee,
            ihr_fee: v.ihr_fee,
            created_lt: v.created_lt,
            body_hash: v.body_hash,
            msg_data: v.msg_data.into(),
        }
    }
}

impl From<RawTransaction> for ton_client::types::RawTransaction {
    fn from(v: RawTransaction) -> Self {
        Self {
            address: v.address.into(),
            utime: v.utime,
            data: v.data,
            transaction_id: v.transaction_id.into(),
            fee: v.fee,
            storage_fee: v.storage_fee,
            other_fee: v.other_fee,
            in_msg: v.in_msg.map(Into::into),
            out_msgs: v.out_msgs.into_iter().map(Into::into).collect(),
        }
    }
}

impl ton_client::client::TonClient for TonClient {
    type Error = anyhow::Error;

    async fn get_masterchain_info(
        &self,
    ) -> Result<ton_client::types::MasterchainInfo, Self::Error> {
        let info = self
            .client
            .clone()
            .oneshot(Specialized::new(BlocksGetMasterchainInfo::default()))
            .await?;

        Ok(info.into())
    }

    async fn look_up_block_by_seqno(
        &self,
        workchain: i32,
        shard: i64,
        seqno: i32,
    ) -> Result<ton_client::types::BlockIdExt, Self::Error> {
        if seqno <= 0 {
            return Err(anyhow!("seqno must be greater than 0"));
        }

        let block: TonBlockIdExt = self
            .client
            .clone()
            .oneshot(BlocksLookupBlock::seqno(TonBlockId::new(
                workchain, shard, seqno,
            )))
            .await?;

        Ok(block.into())
    }

    async fn get_block_header(
        &self,
        workchain: i32,
        shard: i64,
        seqno: i32,
        hashes: Option<(String, String)>,
    ) -> Result<ton_client::types::BlocksHeader, Self::Error> {
        let (root_hash, file_hash) = match hashes {
            Some((root_hash, file_hash)) => (root_hash, file_hash),
            _ => {
                let block = self.look_up_block_by_seqno(workchain, shard, seqno).await?;
                (block.root_hash, block.file_hash)
            }
        };

        let header: BlocksHeader = self
            .client
            .clone()
            .oneshot(BlocksGetBlockHeader::new(TonBlockIdExt {
                workchain,
                shard,
                seqno,
                root_hash,
                file_hash,
            }))
            .await?;

        Ok(header.into())
    }

    async fn get_shards_by_block_id(
        &self,
        block_id: ton_client::types::BlockIdExt,
    ) -> Result<Vec<ton_client::types::BlockIdExt>, Self::Error> {
        let block_id: TonBlockIdExt = block_id.into();
        if block_id.workchain != -1 {
            return Err(anyhow!("workchain must be -1"));
        }

        let shards: BlocksShards = self
            .client
            .clone()
            .oneshot(BlocksGetShards::new(block_id))
            .await?;

        Ok(shards.shards.into_iter().map(Into::into).collect())
    }

    async fn raw_get_account_state_on_block(
        &self,
        address: &str,
        block_id: ton_client::types::BlockIdExt,
    ) -> Result<ton_client::types::RawFullAccountState, Self::Error> {
        let account_address = AccountAddress::new(address)?;

        let state: RawFullAccountState = self
            .client
            .clone()
            .oneshot(WithBlock::new(
                block_id.into(),
                RawGetAccountState::new(account_address),
            ))
            .await?;

        Ok(state.into())
    }

    async fn raw_get_account_state_at_least_block(
        &self,
        address: &str,
        block_id: &ton_client::types::BlockIdExt,
    ) -> Result<ton_client::types::RawFullAccountState, Self::Error> {
        let block_id: TonBlockIdExt = ton_client::types::BlockIdExt::clone(block_id).into();
        let route = Route::Block {
            chain: block_id.workchain,
            criteria: BlockCriteria::Seqno {
                shard: block_id.shard,
                seqno: block_id.seqno,
            },
        };
        let account_address = AccountAddress::new(address)?;

        let state: RawFullAccountState = self
            .client
            .clone()
            .oneshot(Forward::new(
                route,
                RawGetAccountState::new(account_address),
            ))
            .await?;

        Ok(state.into())
    }

    async fn raw_get_account_state_by_transaction(
        &self,
        address: &str,
        transaction_id: ton_client::types::InternalTransactionId,
    ) -> Result<ton_client::types::RawFullAccountState, Self::Error> {
        let account_address = AccountAddress::new(address)?;

        let state: RawFullAccountState = self
            .client
            .clone()
            .oneshot(RawGetAccountStateByTransaction::new(
                account_address,
                transaction_id.into(),
            ))
            .await?;

        Ok(state.into())
    }

    async fn raw_get_account_state(
        &self,
        address: &str,
    ) -> Result<ton_client::types::RawFullAccountState, Self::Error> {
        let account_address = AccountAddress::new(address)?;

        let state: RawFullAccountState = self
            .client
            .clone()
            .oneshot(RawGetAccountState::new(account_address))
            .await?;

        Ok(state.into())
    }

    async fn get_block_transactions_batch(
        &self,
        block: &ton_client::types::BlockIdExt,
        after: Option<&ton_client::types::ShortTxId>,
        reverse: bool,
        limit: i32,
    ) -> Result<(Vec<ton_client::types::ShortTxId>, bool), Self::Error> {
        let block: TonBlockIdExt = block.clone().into();
        let after = after.map(|s| BlocksAccountTransactionId {
            account: s.account.clone(),
            lt: s.lt,
        });

        let txs: BlocksTransactions = self
            .client
            .clone()
            .oneshot(BlocksGetTransactions::unverified(
                block, after, reverse, limit,
            ))
            .await?;

        Ok((
            txs.transactions.into_iter().map(Into::into).collect(),
            txs.incomplete,
        ))
    }

    async fn get_block_raw_transactions_batch(
        &self,
        block: &ton_client::types::BlockIdExt,
        after: Option<&ton_client::types::ShortTxId>,
        reverse: bool,
        limit: i32,
    ) -> Result<(Vec<ton_client::types::RawTransaction>, bool), Self::Error> {
        let block: TonBlockIdExt = block.clone().into();
        let after = after.map(|s| BlocksAccountTransactionId {
            account: s.account.clone(),
            lt: s.lt,
        });

        let txs: BlocksTransactionsExt = self
            .client
            .clone()
            .oneshot(BlocksGetTransactionsExt::unverified(
                block, after, reverse, limit,
            ))
            .await?;

        Ok((
            txs.transactions.into_iter().map(Into::into).collect(),
            txs.incomplete,
        ))
    }

    async fn get_account_transactions_batch(
        &self,
        address: &str,
        from: &ton_client::types::InternalTransactionId,
        limit: i32,
    ) -> Result<
        (
            Vec<ton_client::types::RawTransaction>,
            Option<ton_client::types::InternalTransactionId>,
        ),
        Self::Error,
    > {
        let address = AccountAddress::new(address)?;

        let txs: RawTransactions = self
            .client
            .clone()
            .oneshot(RawGetTransactionsV2::new(
                address,
                from.clone().into(),
                limit,
                false,
            ))
            .await?;

        Ok((
            txs.transactions.into_iter().map(Into::into).collect(),
            txs.previous_transaction_id.map(Into::into),
        ))
    }

    fn get_accounts_in_block_stream(
        &self,
        block: &ton_client::types::BlockIdExt,
    ) -> BoxStream<'static, Result<ton_client::types::InternalAccountAddress, Self::Error>> {
        let chain = block.workchain;
        let stream_map = StreamMap::from_iter(
            [false, true].map(|reverse| (reverse, self.get_block_tx_id_stream(block, reverse))),
        );

        try_stream! {
            let mut last = HashMap::with_capacity(2);

            for await (key, tx) in stream_map {
                let tx = tx?;

                if let Some(addr) = last.get(&!key)
                    && addr == tx.account()
                {
                    return;
                }

                if let Some(addr) = last.get(&key)
                    && addr == tx.account()
                {
                    continue;
                }

                last.insert(key, tx.account().to_owned());

                let address = crate::address::ShardContextAccountAddress::from_str(tx.account())?;
                let address: ton_client::types::InternalAccountAddress =
                    address.into_internal(chain).into();

                yield address;
            }
        }
        .boxed()
    }

    async fn get_shard_account_cell_on_block(
        &self,
        address: &str,
        block: ton_client::types::BlockIdExt,
    ) -> Result<ton_client::types::TvmCell, Self::Error> {
        let address = AccountAddress::new(address)?;
        let cell: TvmCell = self
            .client
            .clone()
            .oneshot(WithBlock::new(
                block.into(),
                GetShardAccountCell::new(address),
            ))
            .await?;

        Ok(cell.into())
    }

    async fn get_shard_account_cell_at_least_block(
        &self,
        address: &str,
        block_id: &ton_client::types::BlockIdExt,
    ) -> Result<ton_client::types::TvmCell, Self::Error> {
        let block_id: TonBlockIdExt = ton_client::types::BlockIdExt::clone(block_id).into();
        let route = Route::Block {
            chain: block_id.workchain,
            criteria: BlockCriteria::Seqno {
                shard: block_id.shard,
                seqno: block_id.seqno,
            },
        };
        let address = AccountAddress::new(address)?;

        let cell: TvmCell = self
            .client
            .clone()
            .oneshot(Forward::new(route, GetShardAccountCell::new(address)))
            .await?;

        Ok(cell.into())
    }

    async fn send_message_returning_hash(&self, body: &str) -> Result<String, Self::Error> {
        self.client
            .clone()
            .oneshot(RawSendMessageReturnHash::new(body.to_string()))
            .map_ok(|res| res.hash)
            .await
    }
}

#[cfg(test)]
mod integration {
    use testcontainers_ton::LocalLiteServer;
    use ton_client::client::TonClient as TonClientTrait;
    use ton_client_util::discover::config::TonConfig;

    use super::*;

    async fn setup() -> (TonClient, LocalLiteServer) {
        let local = LocalLiteServer::new().await.unwrap();
        let config: TonConfig = serde_json::from_value(local.get_config_json().clone()).unwrap();
        let mut client = TonClientBuilder::from_config(config).build().unwrap();
        TonClientTrait::ready(&mut client).await.unwrap();

        (client, local)
    }

    #[tokio::test]
    async fn get_masterchain_info() {
        let (client, _local) = setup().await;

        let info = TonClientTrait::get_masterchain_info(&client).await.unwrap();

        assert_eq!(info.last.workchain, -1);
        assert!(info.last.seqno > 0);
    }

    #[tokio::test]
    async fn look_up_block_and_get_header() {
        let (client, _local) = setup().await;
        let info = TonClientTrait::get_masterchain_info(&client).await.unwrap();

        let block = TonClientTrait::look_up_block_by_seqno(
            &client,
            info.last.workchain,
            info.last.shard,
            info.last.seqno,
        )
        .await
        .unwrap();
        let header = TonClientTrait::get_block_header(
            &client,
            block.workchain,
            block.shard,
            block.seqno,
            Some((block.root_hash.clone(), block.file_hash.clone())),
        )
        .await
        .unwrap();

        assert_eq!(block.workchain, -1);
        assert_eq!(block.seqno, info.last.seqno);
        assert_eq!(header.id.seqno, block.seqno);
        assert!(header.end_lt >= header.start_lt);
    }

    #[tokio::test]
    async fn get_shards_by_block_id() {
        let (client, _local) = setup().await;
        let info = TonClientTrait::get_masterchain_info(&client).await.unwrap();

        let shards = TonClientTrait::get_shards_by_block_id(&client, info.last)
            .await
            .unwrap();

        assert!(!shards.is_empty());
        for shard in &shards {
            assert_eq!(shard.workchain, 0);
        }
    }

    #[tokio::test]
    async fn send_message_returns_error_for_invalid_body() {
        let (client, _local) = setup().await;

        let result = TonClientTrait::send_message_returning_hash(&client, "invalid_boc").await;

        assert!(result.is_err());
    }
}
