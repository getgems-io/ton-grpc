use std::path::PathBuf;
use std::time::Duration;
use futures::{Stream, stream, TryStreamExt};
use anyhow::anyhow;
use serde::de::DeserializeOwned;
use serde_json::Value;
use tower::{Layer, ServiceExt};
use tower::buffer::Buffer;
use tower::load::PeakEwmaDiscover;
use tower::retry::budget::Budget;
use tower::retry::Retry;
use tower::Service;
use url::Url;
use crate::balance::{Balance, BalanceRequest, BlockCriteria, Route};
use crate::block::{InternalTransactionId, RawTransaction, RawTransactions, MasterchainInfo, ShardsResponse, BlockIdExt, AccountTransactionId, BlockTransactions, ShortTxId, RawSendMessage, SmcStack, BlocksLookupBlock, BlockId, BlockHeader, RawGetAccountState, GetMasterchainInfo, BlocksGetShards, GetBlockHeader, GetAccountState, RawGetTransactionsV2, BlocksGetTransactions};
use crate::config::AppConfig;
use crate::discover::{ClientDiscover, CursorClientDiscover};
use crate::error::{ErrorLayer, ErrorService};
use crate::request::{Forward, Requestable};
use crate::retry::RetryPolicy;
use crate::session::SessionRequest;
use crate::request::Routable;

#[derive(Clone)]
pub struct TonClient {
    client: ErrorService<Retry<RetryPolicy, Buffer<Balance, BalanceRequest>>>
}

const MAIN_WORKCHAIN: i64 = -1;
const MAIN_SHARD: i64 = -9223372036854775808;

impl TonClient {
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
        let client = Buffer::new(client, 200000);
        let client = Retry::new(RetryPolicy::new(Budget::new(
            Duration::from_secs(10),
            10,
            0.1
        )), client);
        let client = ErrorLayer::default().layer(client);

        Ok(Self {
            client
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
        let client = Buffer::new(client, 200000);
        let client = Retry::new(RetryPolicy::new(Budget::new(
            Duration::from_secs(10),
            10,
            0.1
        )), client);
        let client = ErrorLayer::default().layer(client);

        Ok(Self {
            client
        })
    }

    pub async fn from_env() -> anyhow::Result<Self> {
        let config = AppConfig::from_env()?;

        tracing::warn!("Ton config url: {}", config.config_url);
        tracing::warn!("Ton config fallback path: {:?}", config.config_path);

        Self::from_url(config.config_url, config.config_path).await
    }

    pub async fn get_masterchain_info(&self) -> anyhow::Result<MasterchainInfo> {
        self.call_ton(GetMasterchainInfo::default()).await
    }

    pub async fn look_up_block_by_seqno(
        &self,
        workchain: i64,
        shard: i64,
        seqno: i32,
    ) -> anyhow::Result<BlockIdExt> {
        if seqno <= 0 {
            return Err(anyhow!("seqno must be greater than 0"));
        }

        let request = BlocksLookupBlock::seqno(BlockId {
            workchain,
            shard,
            seqno
        });

        self.call_ton(request).await
    }

    pub async fn look_up_block_by_lt(
        &self,
        workchain: i64,
        shard: i64,
        lt: i64,
    ) -> anyhow::Result<BlockIdExt> {
        if lt <= 0 {
            return Err(anyhow!("lt must be greater than 0"));
        }

        let request = BlocksLookupBlock::logical_time(
            BlockId::new(workchain, shard, 0), lt
        );

        self.call_ton(request).await
    }

    pub async fn get_shards(&self, workchain: i64, shard: i64, seqno: i32) -> anyhow::Result<ShardsResponse> {
        let block = self
            .look_up_block_by_seqno(workchain, shard, seqno)
            .await?;

        self.call_ton(BlocksGetShards::new(block)).await
    }

    pub async fn get_main_shards(&self, seqno: i32) -> anyhow::Result<ShardsResponse> {
        self.get_shards(MAIN_WORKCHAIN, MAIN_SHARD, seqno).await
    }

    pub async fn get_block_header(
        &self,
        workchain: i64,
        shard: i64,
        seqno: i32,
    ) -> anyhow::Result<BlockHeader> {
        let block = self.look_up_block_by_seqno(workchain, shard, seqno).await?;

        self.call_ton(GetBlockHeader::new(block)).await
    }

    pub async fn raw_get_account_state(&self, address: &str) -> anyhow::Result<Value> {
        let request = RawGetAccountState::new(address.to_string());

        let mut response = self.call_ton(request).await?;

        let code = response["code"].as_str().unwrap_or("");
        let state: &str = if code.is_empty() || code.parse::<i64>().is_ok() {
            if response["frozen_hash"].as_str().unwrap_or("").is_empty() {
                "uninitialized"
            } else {
                "frozen"
            }
        } else {
            "active"
        };

        response["state"] = Value::from(state);
        if let Some(balance) = response["balance"].as_i64() {
            if balance < 0 {
                response["balance"] = Value::from(0);
            }
        }

        Ok(response)
    }

    pub async fn get_account_state(&self, address: &str) -> anyhow::Result<Value> {
        self.call_ton(GetAccountState::new(address.to_owned())).await
    }

    pub async fn raw_get_transactions(
        &self,
        address: &str,
        from_lt: i64,
        from_hash: &str,
    ) -> anyhow::Result<RawTransactions> {
        let request = RawGetTransactionsV2::new(
            address.to_owned(),
            from_hash.to_owned(),
            from_lt
        );

        let response = self.call_ton(request.clone()).await?;

        if response.transactions.len() <= 1 {
            let workchain = request.account_address.workchain_id();
            let forwarded = Forward::new(
                request,
                Route::Block { workchain, criteria: BlockCriteria::LogicalTime(1000000) }
            );
            let archive_response = self.call_ton(forwarded).await?;

            if archive_response.transactions.len() > 1 {
                return Ok(archive_response)
            }
        }

        Ok(response)
    }

    async fn blocks_get_transactions(
        &self,
        block: &BlockIdExt,
        tx: Option<AccountTransactionId>
    ) -> anyhow::Result<BlockTransactions> {
        let request = BlocksGetTransactions::new(block.clone(), tx.unwrap_or_default());

        self.call_ton(request).await
    }

    pub async fn send_message(&self, message: &str) -> anyhow::Result<Value> {
        let request = RawSendMessage::new(message.to_owned());

        self.call_ton(request).await
    }

    pub async fn get_tx_stream(
        &self,
        block: BlockIdExt,
    ) -> impl Stream<Item = anyhow::Result<ShortTxId>> + '_ {
        struct State<'a> {
            last_tx: Option<AccountTransactionId>,
            incomplete: bool,
            block: BlockIdExt,
            this: &'a TonClient
        }

        stream::try_unfold(
            State {
                last_tx: None,
                incomplete: true,
                block,
                this: self
            },
            move |state| {
                async move {
                    if !state.incomplete {
                        return anyhow::Ok(None);
                    }

                    let txs= state.this.blocks_get_transactions(&state.block, state.last_tx).await?;

                    tracing::debug!("got {} transactions", txs.transactions.len());

                    let last_tx = txs.transactions.last().map(Into::into);

                    anyhow::Ok(Some((
                        stream::iter(txs.transactions.into_iter().map(anyhow::Ok)),
                        State {
                            last_tx,
                            incomplete: txs.incomplete,
                            block: state.block,
                            this: state.this
                        },
                    )))
                }
            },
        )
            .try_flatten()
    }

    pub async fn get_account_tx_stream(
        &self,
        address: String,
    ) -> anyhow::Result<impl Stream<Item = anyhow::Result<RawTransaction>> + '_> {
        // TODO[akostylev0] typed
        let account_state = self.raw_get_account_state(&address).await?;
        let ltx = account_state
            .get("last_transaction_id")
            .ok_or_else(||anyhow!("Unexpected missed last_transaction_id"))?;
        let last_tx = serde_json::from_value::<InternalTransactionId>(ltx.to_owned())?;

        Ok(self.get_account_tx_stream_from(address, last_tx))
    }

    pub fn get_account_tx_stream_from(
        &self,
        address: String,
        last_tx: InternalTransactionId,
    ) -> impl Stream<Item = anyhow::Result<RawTransaction>> + '_ {
        struct State<'a> {
            address: String,
            last_tx: InternalTransactionId,
            this: &'a TonClient,
            next: bool
        }

        stream::try_unfold(State { address, last_tx, this: self, next: true }, move |state| async move {
            if !state.next {
                return anyhow::Ok(None);
            }

            let txs = state.this
                .raw_get_transactions(&state.address, state.last_tx.lt, &state.last_tx.hash)
                .await?;

            let mut txs = txs.transactions;
            if txs.len() == 1 {
                anyhow::Ok(Some((
                    stream::iter(txs.into_iter().map(anyhow::Ok)),
                    State {
                        address: state.address,
                        last_tx: state.last_tx,
                        this: state.this,
                        next: false
                    }
                )))
            } else if let Some(next_last_tx) = txs.pop() {
                anyhow::Ok(Some((
                    stream::iter(txs.into_iter().map(anyhow::Ok)),
                    State {
                        address: state.address,
                        last_tx: next_last_tx.transaction_id,
                        this: state.this,
                        next: true
                    }
                )))
            } else {
                anyhow::Ok(None)
            }
        })
            .try_flatten()
    }

    pub async fn run_get_method(&self, address: String, method: String, stack: SmcStack) -> anyhow::Result<Value> {
        self
            .call_session_request(SessionRequest::RunGetMethod { address, method, stack })
            .await
    }

    async fn call_session_request<D : DeserializeOwned>(&self, req: SessionRequest) -> anyhow::Result<D> {
        let response = self.client
            .clone()
            .ready()
            .await?
            .call(req.into())
            .await?;

        serde_json::from_value(response)
            .map_err(anyhow::Error::from)
    }

    async fn call_ton<Req : Routable + Requestable>(&self, req: Req) -> anyhow::Result<Req::Response> {
        let request = req.into_balance_request()?;

        let response = self.client
            .clone()
            .ready()
            .await?
            .call(request)
            .await?;

        serde_json::from_value(response)
            .map_err(anyhow::Error::from)
    }
}
