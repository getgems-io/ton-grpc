mod retry;
mod liteserver;
mod discover;
mod make;
mod client;
mod config;

use anyhow::anyhow;
use futures::TryStreamExt;
use futures::{stream, Stream};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::error::Error;
use std::fmt::{Display, Formatter};
use std::fs::File;
use std::io::BufReader;
use std::path::Path;
use std::time::Duration;
use tower::{Service, ServiceExt};
use tower::balance::p2c::{Balance};
use tower::buffer::Buffer;
use tower::load::PeakEwmaDiscover;
use tower::retry::{Retry};
use tower::retry::budget::Budget;
use crate::client::AsyncClient;
use crate::config::AppConfig;
use crate::discover::DynamicServiceStream;
use crate::make::ClientFactory;
use crate::retry::RetryPolicy;

pub struct ClientBuilder {
    config: Value,
    disable_logging: Option<Value>,
}

impl ClientBuilder {
    pub fn from_json_config(config: &Value) -> Self {
        let full_config = json!({
            "@type": "init",
            "options": {
                "@type": "options",
                "config": {
                    "@type": "config",
                    "config": config.to_string(),
                    "use_callbacks_for_network": false,
                    "blockchain_name": "",
                    "ignore_cache": true
                },
                "keystore_type": {
                    "@type": "keyStoreTypeInMemory"
                }
            }
        });

        Self {
            config: full_config,
            disable_logging: None,
        }
    }

    pub fn from_file<P: AsRef<Path>>(path: P) -> anyhow::Result<Self> {
        let file = File::open(path)?;
        let reader = BufReader::new(file);
        let config: Value = serde_json::from_reader(reader)?;

        Ok(ClientBuilder::from_json_config(&config))
    }

    pub fn disable_logging(&mut self) -> &mut Self {
        self.disable_logging = Some(json!({
            "@type": "setLogVerbosityLevel",
            "new_verbosity_level": 0
        }));

        self
    }

    pub async fn build(&self) -> anyhow::Result<AsyncClient> {
        #[derive(Deserialize)]
        struct Void {}

        let client = AsyncClient::new();
        if let Some(ref disable_logging) = self.disable_logging {
            client.execute(disable_logging.clone()).await?;
        }

        client.execute(self.config.clone()).await?;

        Ok(client)
    }
}

const MAIN_WORKCHAIN: i64 = -1;
const MAIN_SHARD: i64 = -9223372036854775808;

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(tag = "@type", rename = "blocks.shortTxId")]
pub struct ShortTxId {
    pub account: String,
    pub hash: String,
    pub lt: String,
    pub mode: u8,
}

#[derive(Debug, Deserialize)]
pub struct TonError {
    code: i32,
    message: String,
}

impl Display for TonError {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "Ton api error occurred with code {}, message {}",
            self.code, self.message
        )
    }
}

impl Error for TonError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        None
    }
}

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(tag = "@type", rename = "ton.blockIdExt")]
pub struct BlockIdExt {
    pub workchain: i64,
    pub shard: String,
    pub seqno: u64,
    pub root_hash: String,
    pub file_hash: String,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(tag = "@type", rename = "blocks.masterchainInfo")]
pub struct MasterchainInfo {
    pub init: BlockIdExt,
    pub last: BlockIdExt,
    pub state_root_hash: String,
}

#[derive(Deserialize, Serialize, Debug, Clone)]
#[serde(tag = "@type", rename = "internal.transactionId")]
pub struct InternalTransactionId {
    pub hash: String,
    pub lt: String,
}

#[derive(Deserialize, Serialize, Debug)]
#[serde(tag = "@type", rename = "accountAddress")]
pub struct AccountAddress {
    account_address: String,
}

#[derive(Deserialize, Serialize, Debug)]
#[serde(tag = "@type", rename = "raw.message")]
pub struct RawMessage {
    source: AccountAddress,
    destination: AccountAddress,
    value: String,
    fwd_fee: String,
    ihr_fee: String,
    created_lt: String,
    body_hash: String,
    msg_data: Value, // @todo maybe only msg.dataRaw
                     // @todo deserialize boc
}

#[derive(Deserialize, Serialize, Debug)]
#[serde(tag = "@type", rename = "raw.transaction")]
pub struct RawTransaction {
    pub utime: i64,
    pub data: String,
    pub transaction_id: InternalTransactionId,
    pub fee: String,
    pub storage_fee: String,
    pub other_fee: String,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub in_msg: Option<RawMessage>,
    pub out_msgs: Vec<RawMessage>,
}

#[derive(Deserialize, Serialize, Debug)]
pub struct RawTransactions {
    transactions: Vec<RawTransaction>,
    previous_transaction_id: InternalTransactionId,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(tag = "@type", rename = "blocks.getMasterchainInfo")]
pub struct GetMasterchainInfo {}

#[derive(Debug, Serialize, Deserialize)]
pub struct ShardsResponse {
    pub shards: Vec<BlockIdExt>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct TransactionsResponse {
    pub id: BlockIdExt,
    pub incomplete: bool,
    pub req_count: u32,
    pub transactions: Vec<ShortTxId>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(tag = "@type", rename = "blocks.accountTransactionId")]
pub struct AccountTransactionId {
    pub account: String,
    #[serde(skip_serializing_if = "String::is_empty")]
    pub lt: String,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(tag = "@type", rename = "raw.sendMessage")]
pub struct RawSendMessage {
    pub body: String,
}

impl From<&ShortTxId> for AccountTransactionId {
    fn from(v: &ShortTxId) -> Self {
        AccountTransactionId {
            account: v.account.clone(),
            lt: v.lt.clone(),
        }
    }
}

pub type ServiceError = Box<(dyn Error + Sync + Send)>;
pub type TonNaive = AsyncClient;
pub type TonBalanced = Retry<RetryPolicy, Buffer<Balance<PeakEwmaDiscover<DynamicServiceStream>, Value>, Value>>;

#[derive(Clone)]
pub struct Ton<S> where S : Service<Value, Response = Value, Error = ServiceError> {
    service: S
}

impl Ton<TonBalanced> {
    pub async fn balanced() -> anyhow::Result<Self> {
        let config = AppConfig::from_env()?;

        let discover = DynamicServiceStream::new(
            config.config_url,
            Duration::from_secs(60)
        );

        let emwa = PeakEwmaDiscover::new(
            discover,
            Duration::from_millis(300),
            Duration::from_secs(10),
            tower::load::CompleteOnResponse::default(),
        );

        let ton = Balance::new(emwa);
        let ton = Buffer::new(ton, 200000);
        let ton = Retry::new(RetryPolicy::new(Budget::new(
            Duration::from_secs(10),
            10,
            0.1
        )), ton);

        let ton = Self {
            service: ton
        };

        ton.get_masterchain_info().await?;

        Ok(ton)
    }
}

impl<S> Ton<S> where S : Service<Value, Response = Value, Error = ServiceError> + Clone
{
    pub fn new(service: S) -> Self {
        Self { service }
    }

    pub async fn get_masterchain_info(&self) -> anyhow::Result<MasterchainInfo> {
        let query = json!(GetMasterchainInfo {});

        let response = self.call(query).await?;

        Ok(serde_json::from_value(response)?)
    }

    pub async fn look_up_block_by_seqno(
        &self,
        workchain: i64,
        shard: i64,
        seqno: u64,
    ) -> anyhow::Result<Value> {
        self.look_up_block(workchain, shard, seqno, 0).await
    }

    pub async fn look_up_block_by_lt(
        &self,
        workchain: i64,
        shard: i64,
        lt: i64,
    ) -> anyhow::Result<Value> {
        self.look_up_block(workchain, shard, 0, lt).await
    }

    pub async fn get_shards(&self, master_seqno: u64) -> anyhow::Result<ShardsResponse> {
        let block = self
            .look_up_block(MAIN_WORKCHAIN, MAIN_SHARD, master_seqno, 0)
            .await?;

        let request = json!({
            "@type": "blocks.getShards",
            "id": block
        });

        let response = self.call(request).await?;

        Ok(serde_json::from_value(response)?)
    }

    pub async fn get_block_header(
        &self,
        workchain: i64,
        shard: i64,
        seqno: u64,
    ) -> anyhow::Result<Value> {
        let block = self.look_up_block(workchain, shard, seqno, 0).await?;

        let request = json!({
            "@type": "blocks.getBlockHeader",
            "id": block
        });

        self.call(request).await
    }

    pub async fn raw_get_account_state(&self, address: &str) -> anyhow::Result<Value> {
        let request = json!({
            "@type": "raw.getAccountState",
            "account_address": {
                "account_address": address
            }
        });

        let mut response = self.call(request).await?;

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
        let request = json!({
            "@type": "getAccountState",
            "account_address": {
                "account_address": address
            }
        });

        self.call(request).await
    }

    pub async fn raw_get_transactions(
        &self,
        address: &str,
        from_lt: &str,
        from_hash: &str,
    ) -> anyhow::Result<RawTransactions> {
        let request = json!({
            "@type": "raw.getTransactions",
            "account_address": {
                "account_address": address
            },
            "from_transaction_id": {
                "@type": "internal.transactionId",
                "lt": from_lt,
                "hash": from_hash
            }
        });

        let response = self.call(request).await?;

        Ok(serde_json::from_value(response)?)
    }

    async fn blocks_get_transactions(
        &self,
        block: &BlockIdExt,
        count: u32,
    ) -> anyhow::Result<TransactionsResponse> {
        self.blocks_get_transactions_after(
            block,
            count,
            AccountTransactionId {
                account: "AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA=".to_string(),
                lt: "".to_string(),
            },
        )
        .await
    }

    async fn blocks_get_transactions_after(
        &self,
        block: &BlockIdExt,
        count: u32,
        tx: AccountTransactionId,
    ) -> anyhow::Result<TransactionsResponse> {
        let request = json!({
            "@type": "blocks.getTransactions",
            "id": block,
            "mode": 7 + 128,
            "count": count,
            "after": tx,
        });

        let response = self.call(request).await?;

        Ok(serde_json::from_value(response)?)
    }

    async fn look_up_block(
        &self,
        workchain: i64,
        shard: i64,
        seqno: u64,
        lt: i64,
    ) -> anyhow::Result<Value> {
        let mut mode: i32 = 0;
        if seqno > 0 {
            mode += 1
        }
        if lt > 0 {
            mode += 2
        }

        let request = json!({
            "@type": "blocks.lookupBlock",
            "mode": mode,
            "id": {
                "@type": "ton.blockId",
                "workchain": workchain,
                "shard": shard,
                "seqno": seqno
            },
            "lt": lt
        });

        self.call(request).await
    }

    pub async fn send_message(&self, message: &str) -> anyhow::Result<Value> {
        let request = json!(RawSendMessage {
            body: message.to_string()
        });

        self.call(request).await
    }

    pub async fn get_tx_stream(
        &self,
        block: BlockIdExt,
    ) -> impl Stream<Item = anyhow::Result<ShortTxId>> + '_ {
        struct State<'a, S : Service<Value, Response = Value, Error = ServiceError>> {
            last_tx: Option<AccountTransactionId>,
            incomplete: bool,
            block: BlockIdExt,
            this: &'a Ton<S>
        }

        let this = self;
        stream::try_unfold(
            State {
                last_tx: None,
                incomplete: true,
                block,
                this
            },
            move |state| {
                async move {
                    if !state.incomplete {
                        return anyhow::Ok(None);
                    }

                    let txs= if let Some(tx) = state.last_tx {
                        state.this.blocks_get_transactions_after(&state.block, 30, tx).await?
                    } else {
                        state.this.blocks_get_transactions(&state.block, 30).await?
                    };

                    tracing::debug!("got {} transactions", txs.transactions.len());

                    let last_tx = txs.transactions.last().map(AccountTransactionId::from);

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
        let ltx = account_state
            .get("last_transaction_id")
            .ok_or_else(||anyhow!("Unexpected missed last_transaction_id"))?;
        let last_tx = serde_json::from_value::<InternalTransactionId>(ltx.to_owned())?;

        return Ok(self.get_account_tx_stream_from(address, last_tx));
    }

    pub fn get_account_tx_stream_from(
        &self,
        address: String,
        last_tx: InternalTransactionId,
    ) -> impl Stream<Item = anyhow::Result<RawTransaction>> + '_ {
        struct State<'a, S : Service<Value, Response = Value, Error = ServiceError>> {
            address: String,
            last_tx: InternalTransactionId,
            this: &'a Ton<S>
        }

        let this = self;
        stream::try_unfold(State { address, last_tx, this }, move |state| async move {
            let txs = state.this
                .raw_get_transactions(&state.address, &state.last_tx.lt, &state.last_tx.hash)
                .await?;

            if let Some(last_tx) = txs.transactions.last() {
                let tx_id = last_tx.transaction_id.clone();
                anyhow::Ok(Some((
                    stream::iter(txs.transactions.into_iter().map(anyhow::Ok)),
                    State {
                        address: state.address,
                        last_tx: tx_id,
                        this: state.this
                    },
                )))
            } else {
                anyhow::Ok(None)
            }
        })
        .try_flatten()
    }

    async fn call(&self, request: Value) -> anyhow::Result<Value> {
        let mut ton = self.clone();
        let ready = ton.service.ready().await.map_err(|e| anyhow!(e))?;
        let call = ready.call(request).await.map_err(|e| anyhow!(e))?;

        Ok(call)
    }
}
