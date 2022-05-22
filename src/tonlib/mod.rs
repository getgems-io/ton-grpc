use async_stream::stream;
use futures::StreamExt;
use futures::{pin_mut, stream, FutureExt, Stream};
use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::borrow::Borrow;
use std::collections::HashMap;
use std::error::Error;
use std::fmt::{format, write, Display, Formatter};
use std::fs::File;
use std::io::BufReader;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::Duration;
use tokio::sync::oneshot::error::RecvError;
use tokio::sync::Semaphore;
use tonlibjson_rs::Client;
use uuid::Uuid;

pub struct ClientBuilder {
    config: Value,
    disable_logging: Option<Value>,
}

impl ClientBuilder {
    pub fn from_json_config(config: &Value) -> anyhow::Result<Self> {
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

        Ok(Self {
            config: full_config,
            disable_logging: None,
        })
    }

    pub fn from_file<P: AsRef<Path>>(path: P) -> anyhow::Result<Self> {
        let file = File::open(path)?;
        let reader = BufReader::new(file);
        let config: Value = serde_json::from_reader(reader)?;

        return ClientBuilder::from_json_config(&config);
    }

    pub fn disable_logging(&mut self) -> &mut Self {
        self.disable_logging = Some(json!({
            "@type": "setLogVerbosityLevel",
            "new_verbosity_level": 0
        }));

        self
    }

    pub async fn build(&self) -> anyhow::Result<AsyncClient> {
        let client = AsyncClient::new();
        if let Some(ref disable_logging) = self.disable_logging {
            client.execute(disable_logging).await?;
        }

        client.execute(&self.config).await?;

        Ok(client)
    }
}

const MAIN_WORKCHAIN: i64 = -1;
const MAIN_SHARD: i64 = -9223372036854775808;

pub trait TlType {
    fn tl_type() -> String;
}

#[derive(Debug, Serialize, Deserialize, Clone)]
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

impl ShortTxId {
    fn ton_type() -> String {
        return "ton.shortTxId".to_string();
    }
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct BlockIdExt {
    pub workchain: i64,
    pub shard: String,
    pub seqno: u64,
    pub root_hash: String,
    pub file_hash: String,
}

impl TlType for BlockIdExt {
    fn tl_type() -> String {
        return "ton.blockIdExt".to_string();
    }
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct MasterchainInfoResponse {
    pub init: BlockIdExt,
    pub last: BlockIdExt,
    pub state_root_hash: String,
}

impl TlType for MasterchainInfoResponse {
    fn tl_type() -> String {
        return "blocks.masterchainInfo".to_string();
    }
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(tag = "@type")]
pub enum TlBlock {
    #[serde(rename = "ton.blockIdExt")]
    BlockIdExt(BlockIdExt),
    #[serde(rename = "blocks.shortTxId")]
    ShortTxId(ShortTxId),
    #[serde(rename = "blocks.getMasterchainInfo")]
    GetMasterchainInfoRequest,
    // #[serde(rename = "blocks.masterchainInfo")]
    // GetMasterchainInfoResponse(MasterchainInfoResponse),
    #[serde(rename = "blocks.accountTransactionId")]
    AccountTransactionId(AccountTransactionId),
}

#[derive(Debug, Serialize, Deserialize)]
struct ShardsResponse {
    shards: Vec<BlockIdExt>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct TransactionsResponse {
    pub id: BlockIdExt,
    pub incomplete: bool,
    pub req_count: u32,
    pub transactions: Vec<ShortTxId>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct AccountTransactionId {
    pub account: String,
    pub lt: String,
}

impl From<&ShortTxId> for AccountTransactionId {
    fn from(v: &ShortTxId) -> Self {
        AccountTransactionId {
            account: v.account.clone(),
            lt: v.lt.clone(),
        }
    }
}

impl TlType for AccountTransactionId {
    fn tl_type() -> String {
        return "blocks.accountTransactionId".to_string();
    }
}

pub struct AsyncClient {
    client: Arc<Client>,
    responses: Arc<Mutex<HashMap<String, tokio::sync::oneshot::Sender<Value>>>>,
    semaphore: Semaphore,
}

impl AsyncClient {
    pub fn new() -> Self {
        let client = Arc::new(Client::new());
        let client_recv = client.clone();
        let semaphore = tokio::sync::Semaphore::new(500);

        let responses: Arc<Mutex<HashMap<String, tokio::sync::oneshot::Sender<Value>>>> =
            Arc::new(Mutex::new(HashMap::new()));
        let responses_rcv = Arc::clone(&responses);

        let _ = thread::spawn(move || {
            let timeout = Duration::from_secs(20);
            loop {
                if let Ok(packet) = client_recv.receive(timeout) {
                    if let Ok(json) = serde_json::from_str::<Value>(packet) {
                        if let Some(Value::String(ref id)) = json.get("@extra") {
                            let mut resps = responses_rcv.lock().unwrap();
                            let s = resps.remove::<String>(id);
                            drop(resps);
                            if let Some(s) = s {
                                let _ = s.send(json);
                            }
                        }
                    }
                }
            }
        });

        return AsyncClient {
            client,
            responses,
            semaphore,
        };
    }

    async fn send(&self, request: serde_json::Value) -> () {
        let _ = self.client.send(&request.to_string());
    }

    async fn execute(&self, request: &Value) -> anyhow::Result<Value> {
        let mut request = request.clone();

        let id = Uuid::new_v4().to_string();
        request["@extra"] = json!(id);
        let (tx, rx) = tokio::sync::oneshot::channel::<Value>();
        self.responses.lock().unwrap().insert(id, tx);

        let x = request.to_string();
        let permit = self.semaphore.acquire();
        println!("{} available permits", self.semaphore.available_permits());
        let _ = self.client.send(&x);

        let mut value = rx.await?;
        drop(permit);

        let mut obj = value.as_object_mut().unwrap();
        let _ = obj.remove("@extra");

        let value = Value::Object(obj.clone());

        return Ok(value);
    }

    async fn execute_typed<T: DeserializeOwned>(
        &self,
        request: &serde_json::Value,
    ) -> anyhow::Result<T> {
        let mut request = request.clone();

        let id = Uuid::new_v4().to_string();
        request["@extra"] = json!(id);
        let (tx, rx) = tokio::sync::oneshot::channel::<Value>();
        self.responses.lock().unwrap().insert(id, tx);

        let x = request.to_string();
        let permit = self.semaphore.acquire().await.unwrap();
        println!("{} available permits", self.semaphore.available_permits());
        // println!("{:#?}", x);
        let _ = self.client.send(&x);

        let value = rx.await?;
        // println!("{:#?}", value);

        drop(permit);

        if value["@type"] == "error" {
            return match serde_json::from_value::<TonError>(value) {
                Ok(e) => Err(anyhow::Error::from(e)),
                Err(e) => Err(anyhow::Error::from(e)),
            };
        }

        return serde_json::from_value::<T>(value).map_err(anyhow::Error::from);
    }

    pub async fn get_masterchain_info(&self) -> anyhow::Result<Value> {
        let query = json!(TlBlock::GetMasterchainInfoRequest);

        return self.execute(&query).await;
    }

    pub async fn look_up_block_by_seqno(
        &self,
        workchain: i64,
        shard: i64,
        seqno: u64,
    ) -> anyhow::Result<Value> {
        return self.look_up_block(workchain, shard, seqno, 0).await;
    }

    pub async fn look_up_block_by_lt(&self, workchain: i64, shard: i64, lt: i64) -> anyhow::Result<Value> {
        return self.look_up_block(workchain, shard, 0, lt).await;
    }

    pub async fn get_shards(&self, master_seqno: u64) -> anyhow::Result<Value> {
        let block = self
            .look_up_block(MAIN_WORKCHAIN, MAIN_SHARD, master_seqno, 0)
            .await?;
        let request = json!({
            "@type": "blocks.getShards",
            "id": block
        });

        return self.execute(&request).await;
    }

    pub async fn get_block_header(&self, workchain: i64, shard: i64, seqno: u64) -> anyhow::Result<Value> {
        let block = self
            .look_up_block(workchain, shard, seqno, 0)
            .await?;

        let request = json!({
            "@type": "blocks.getBlockHeader",
            "id": block
        });

        return self.execute(&request).await;
    }

    pub async fn get_tx_stream(&self, block: BlockIdExt) -> anyhow::Result<impl Stream<Item = ShortTxId> + '_> {
        struct State {
            last_tx: Option<AccountTransactionId>,
            incomplete: bool,
            block: BlockIdExt,
        }

        return Ok(stream::unfold(
            State {
                last_tx: None,
                incomplete: true,
                block,
            },
            {
                move |state: State| async move {
                    if state.incomplete == false {
                        return None;
                    }

                    let txs;
                    if let Some(tx) = state.last_tx {
                        txs = self
                            ._get_transactions_after(&state.block, 30, tx)
                            .await
                            .unwrap();
                    } else {
                        txs = self._get_transactions(&state.block, 30).await.unwrap();
                    }

                    println!("got {} transactions", txs.transactions.len());

                    let last_tx = txs.transactions.last().map(AccountTransactionId::from);

                    return Some((
                        stream::iter(txs.transactions),
                        State {
                            last_tx,
                            incomplete: txs.incomplete,
                            block: state.block,
                        },
                    ));
                }
            },
        )
        .flatten());
    }

    async fn _get_transactions(
        &self,
        block: &BlockIdExt,
        count: u32,
    ) -> anyhow::Result<TransactionsResponse> {
        let request = json!({
            "@type": "blocks.getTransactions",
            "id": block,
            "mode": 7,
            "count": count,
            "after": {
                "@type": "blocks.accountTransactionId",
                "account": "AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA="
            },
        });

        return self.execute_typed(&request).await;
    }

    async fn _get_transactions_after(
        &self,
        block: &BlockIdExt,
        count: u32,
        tx: AccountTransactionId,
    ) -> anyhow::Result<TransactionsResponse> {
        let block = TlBlock::BlockIdExt(block.clone());
        let tx = TlBlock::AccountTransactionId(tx);

        let request = json!({
            "@type": "blocks.getTransactions",
            "id": block,
            "mode": 7 + 128,
            "count": count,
            "after": tx,
        });

        return self.execute_typed(&request).await;
    }

    async fn look_up_block(&self, workchain: i64, shard: i64, seqno: u64, lt: i64) -> anyhow::Result<Value> {
        let mut mode: i32 = 0;
        if seqno > 0 {
            mode += 1
        }
        if lt > 0 {
            mode += 2
        }

        let query = json!({
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

        return self.execute(&query).await;
    }
}


pub fn base64_to_hex(b: &str) -> anyhow::Result<String> {
    let bytes = base64::decode(b)?;
    let hex = hex::encode(bytes);

    return Ok(hex)
}