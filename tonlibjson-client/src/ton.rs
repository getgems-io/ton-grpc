use std::path::PathBuf;
use std::time::Duration;
use futures::{Stream, stream, TryStreamExt};
use anyhow::anyhow;
use serde_json::Value;
use tower::Layer;
use tower::buffer::Buffer;
use tower::load::PeakEwmaDiscover;
use tower::retry::budget::Budget;
use tower::retry::Retry;
use url::Url;
use crate::balance::{Balance, BalanceRequest, BlockCriteria, Route};
use crate::block::{InternalTransactionId, RawTransaction, RawTransactions, MasterchainInfo, BlocksShards, BlockIdExt, AccountTransactionId, BlocksTransactions, ShortTxId, RawSendMessage, SmcStack, AccountAddress, BlocksGetTransactions, BlocksLookupBlock, BlockId, BlocksGetShards, BlocksGetBlockHeader, BlockHeader, RawGetTransactionsV2, RawGetAccountState, GetAccountState, GetMasterchainInfo, SmcMethodId, GetShardAccountCell, Cell, RawFullAccountState, WithBlock};
use crate::config::AppConfig;
use crate::discover::{ClientDiscover, CursorClientDiscover};
use crate::error::{ErrorLayer, ErrorService};
use crate::request::Forward;
use crate::retry::RetryPolicy;
use crate::session::RunGetMethod;
use crate::request::Callable;

#[derive(Clone)]
pub struct TonClient {
    client: ErrorService<Retry<RetryPolicy, Buffer<Balance, BalanceRequest>>>
}

const MAIN_CHAIN: i32 = -1;
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
        from_lt: i64,
        from_hash: &str,
    ) -> anyhow::Result<RawTransactions> {
        let mut client = self.client.clone();

        let address = AccountAddress::new(address)?;
        let chain = address.chain_id();

        let request = RawGetTransactionsV2::new(
            address,
            InternalTransactionId::new(from_hash.to_owned(), from_lt)
        );
        let response = request.clone().call(&mut client).await?;

        if response.transactions.len() <= 1 {
            let forwarded = Forward::new(
                request,
                Route::Block { chain, criteria: BlockCriteria::Seqno(1) }
            );
            let archive_response = forwarded.call(&mut client).await?;

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
    ) -> anyhow::Result<BlocksTransactions> {
        let mut client = self.client.clone();

        BlocksGetTransactions::new(
            block.to_owned(),
            tx.unwrap_or_default()
        ).call(&mut client).await
    }

    pub async fn send_message(&self, message: &str) -> anyhow::Result<Value> {
        let mut client = self.client.clone();

        RawSendMessage::new(message.to_string()).call(&mut client).await
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
        let account_state = self.raw_get_account_state(&address).await?;

        return Ok(self.get_account_tx_stream_from(address, account_state.last_transaction_id.unwrap_or_default()));
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
}
