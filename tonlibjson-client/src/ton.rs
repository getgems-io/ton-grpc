use std::collections::Bound;
use std::ops::{RangeBounds};
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
use crate::balance::{Balance, BalanceRequest};
use crate::block::{InternalTransactionId, RawTransaction, RawTransactions, MasterchainInfo, BlocksShards, BlockIdExt, AccountTransactionId, BlocksTransactions, ShortTxId, RawSendMessage, SmcStack, AccountAddress, BlocksGetTransactions, BlocksLookupBlock, BlockId, BlocksGetShards, BlocksGetBlockHeader, BlockHeader, RawGetTransactionsV2, RawGetAccountState, GetAccountState, GetMasterchainInfo, SmcMethodId, GetShardAccountCell, Cell, RawFullAccountState, WithBlock, RawGetAccountStateByTransaction, GetShardAccountCellByTransaction};
use crate::config::AppConfig;
use crate::discover::{ClientDiscover, CursorClientDiscover};
use crate::error::{ErrorLayer, ErrorService};
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

    pub fn get_account_tx_stream(
        &self,
        address: &str,
    ) -> impl Stream<Item = anyhow::Result<RawTransaction>> + 'static {
        self.get_account_tx_stream_from(address, None)
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
        let stream = stream.try_take_while(move |x| std::future::ready(Ok({
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
        })));

        stream
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
}
