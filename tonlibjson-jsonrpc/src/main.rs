use std::future;
use std::sync::Arc;
use std::time::Duration;
use anyhow::anyhow;
use axum::{Json, Router, routing::post};
use futures::future::Either::{Left, Right};
use futures::{TryStreamExt, StreamExt};
use serde_json::{json, Value};
use serde::{Deserialize, Serialize};
use tracing::debug;
use tonlibjson_tokio::{BlockIdExt, InternalTransactionId, MasterchainInfo, RawTransaction, ShortTxId, Ton, TonBalanced};

#[derive(Deserialize, Debug)]
struct LookupBlockParams {
    workchain: i64,
    shard: String,
    seqno: Option<u64>,
    lt: Option<i64>,
    unixtime: Option<u64>
}

#[derive(Deserialize)]
struct ShardsParams {
    seqno: u64
}

#[allow(dead_code)]
#[derive(Deserialize)]
struct BlockHeaderParams {
    workchain: i64,
    shard: String,
    seqno: u64,
    root_hash: Option<String>,
    file_hash: Option<String>
}

#[allow(dead_code)]
#[derive(Deserialize)]
struct BlockTransactionsParams {
    workchain: i64,
    shard: String,
    seqno: u64,
    root_hash: Option<String>,
    file_hash: Option<String>,
    after_lt: Option<i64>,
    after_hash: Option<String>,
    count: Option<u8>
}

#[derive(Deserialize, Debug)]
struct AddressParams {
    address: String
}

#[allow(dead_code)]
#[derive(Deserialize)]
struct TransactionsParams {
    address: String,
    limit: Option<u16>,
    lt: Option<String>,
    hash: Option<String>,
    to_lt: Option<String>,
    archival: Option<bool>
}

#[derive(Deserialize, Debug)]
struct SendBocParams {
    boc: String
}

#[derive(Deserialize)]
#[serde(tag = "method")]
enum Method {
    #[serde(rename = "lookupBlock")]
    LookupBlock { params: LookupBlockParams },
    #[serde(rename = "shards")]
    Shards { params: ShardsParams },
    #[serde(rename = "getBlockHeader")]
    BlockHeader { params: BlockHeaderParams },
    #[serde(rename = "getBlockTransactions")]
    BlockTransactions { params: BlockTransactionsParams },
    #[serde(rename = "getAddressInformation")]
    AddressInformation { params: AddressParams },
    #[serde(rename = "getExtendedAddressInformation")]
    ExtendedAddressInformation { params: AddressParams },
    #[serde(rename = "getTransactions")]
    Transactions { params: TransactionsParams },
    #[serde(rename = "sendBoc")]
    SendBoc { params: SendBocParams },
    #[serde(rename = "getMasterchainInfo")]
    MasterchainInfo
}

type JsonRequestId = Option<Value>;

#[allow(dead_code)]
#[derive(Deserialize)]
struct JsonRequest {
    jsonrpc: Option<String>,
    id: JsonRequestId,
    #[serde(flatten)]
    method: Method
}

#[derive(Debug, Serialize)]
struct JsonError {
    code: i32,
    message: String
}

#[derive(Debug, Serialize)]
struct JsonResponse {
    ok: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    error: Option<JsonError>,
    #[serde(skip_serializing_if = "Option::is_none")]
    result: Option<Value>,
    jsonrpc: String,
    id: JsonRequestId
}

impl JsonResponse {
    fn new(id: JsonRequestId, result: Value) -> Self {
        Self {
            ok: true,
            result: Some(result),
            error: None,
            jsonrpc: "2.0".to_string(),
            id
        }
    }

    fn error(id: JsonRequestId, e: anyhow::Error) -> Self {
        Self {
            ok: false,
            result: None,
            error: Some(JsonError { code: -32603, message: e.to_string() }),
            jsonrpc: "2.0".to_string(),
            id
        }
    }
}

struct RpcServer {
    client: Ton<TonBalanced>
}

type RpcResponse<T> = anyhow::Result<T>;

impl RpcServer {
    async fn master_chain_info(&self) -> RpcResponse<MasterchainInfo> {
        self.client.get_masterchain_info().await
    }

    async fn lookup_block(&self, params: LookupBlockParams) -> RpcResponse<Value> {
        let workchain = params.workchain;
        let shard = params.shard.parse::<i64>()?;

        match (params.seqno, params.lt, params.unixtime) {
            (Some(seqno), None, None) if seqno > 0 => self.client.look_up_block_by_seqno(workchain, shard, seqno).await,
            (None, Some(lt), None) if lt > 0 => self.client.look_up_block_by_lt(workchain, shard, lt).await,
            (None, None, Some(_)) => Err(anyhow!("unixtime is not supported")),
            _ => Err(anyhow!("seqno or lt or unixtime must be provided"))
        }
    }

    async fn shards(&self, params: ShardsParams) -> RpcResponse<Value> {
        let response = self.client.get_shards(params.seqno).await?;

        Ok(serde_json::to_value(response)?)
    }

    async fn get_block_header(&self, params: BlockHeaderParams) -> RpcResponse<Value> {
        let shard = params.shard.parse::<i64>()?;

        self.client.get_block_header(
            params.workchain,
            shard,
            params.seqno
        ).await
    }

    async fn get_block_transactions(&self, params: BlockTransactionsParams) -> RpcResponse<Value> {
        let shard = params.shard.parse::<i64>()?;
        let count = params.count.unwrap_or(200);

        let block_json = self.client.look_up_block_by_seqno(params.workchain, shard, params.seqno).await?;

        let block = serde_json::from_value::<BlockIdExt>(block_json)?;

        let stream = self.client.get_tx_stream(block.clone()).await;
        let txs: Vec<ShortTxId> = stream.try_collect().await?;

        let txs: Vec<ShortTxId> = txs.into_iter()
            .map(|tx: ShortTxId| {
                ShortTxId {
                    account: format!("{}:{}", block.workchain, base64_to_hex(&tx.account).unwrap()),
                    hash: tx.hash,
                    lt: tx.lt,
                    mode: tx.mode
                }
            }).collect();


        Ok(json!({
                "@type": "blocks.transactions",
                "id": &block,
                "incomplete": false,
                "req_count": count,
                "transactions": &txs
            }))
    }

    async fn get_address_information(&self, params: AddressParams) -> RpcResponse<Value> {
        self.client.raw_get_account_state(&params.address).await
    }

    async fn get_extended_address_information(&self, params: AddressParams) -> RpcResponse<Value> {
        self.client.get_account_state(&params.address).await
    }

    async fn get_transactions(&self, params: TransactionsParams) -> RpcResponse<Value> {
        let address = params.address;
        let count = params.limit.unwrap_or(10);
        let max_lt = params.to_lt.and_then(|x| x.parse::<i64>().ok());
        let lt = params.lt;
        let hash = params.hash.and_then(|h| {
            if h.len() == 64 {
                hex_to_base64(&h).ok()
            } else {
                Some(h)
            }
        });

        let stream = match (lt, hash) {
            (Some(lt), Some(hash)) => Left(
                self.client.get_account_tx_stream_from(address, InternalTransactionId {hash, lt})
            ),
            _ => Right(
                self.client.get_account_tx_stream(address).await?
            )
        };
        let stream = match max_lt {
            Some(to_lt) => Left(stream.try_take_while(move |tx: &RawTransaction|
                future::ready(Ok(tx.transaction_id.lt.parse::<i64>().unwrap() > to_lt))
            )),
            _ => Right(stream)
        };

        let txs: Vec<RawTransaction> = stream
            .take(count as usize)
            .try_collect()
            .await?;


        let mut response = serde_json::to_value(txs)?;

        // TODO meh
        let mapped: Vec<&mut Value> = response.as_array_mut().unwrap().iter_mut().map(|x| {
            if let Some(in_msg) = x.get_mut("in_msg") {
                if let Some(source) = in_msg.get_mut("source") {
                    *source = source.get("account_address").unwrap().clone()
                }

                if let Some(destination) = in_msg.get_mut("destination") {
                    *destination = destination.get("account_address").unwrap().clone()
                }
            }

            if let Some(out_msgs) = x.get_mut("out_msgs") {
                *out_msgs = Value::Array(out_msgs.as_array_mut().unwrap().iter_mut().map(|out_msg| {
                    if let Some(source) = out_msg.get_mut("source") {
                        *source = source.get("account_address").unwrap().clone()
                    }

                    if let Some(destination) = out_msg.get_mut("destination") {
                        *destination = destination.get("account_address").unwrap().clone()
                    }
                    out_msg.clone()
                }).collect())
            }

            x
        }).collect();

        Ok(serde_json::to_value(mapped)?)
    }

    async fn send_boc(&self, params: SendBocParams) -> RpcResponse<Value> {
        let boc = base64::decode(params.boc)?;
        let b64 = base64::encode(boc);

        self.client.send_message(&b64).await
    }
}

async fn dispatch_method(Json(payload): Json<JsonRequest>, rpc: Arc<RpcServer>) -> Json<JsonResponse> {
    let result = match payload.method {
        Method::MasterchainInfo => rpc.master_chain_info().await.and_then(|x| Ok(serde_json::to_value(x)?)),
        Method::LookupBlock { params } => rpc.lookup_block(params).await.and_then(|x| Ok(serde_json::to_value(x)?)),
        Method::Shards { params } => rpc.shards(params).await.and_then(|x| Ok(serde_json::to_value(x)?)),
        Method::BlockHeader { params } => rpc.get_block_header(params).await.and_then(|x| Ok(serde_json::to_value(x)?)),
        Method::BlockTransactions { params } => rpc.get_block_transactions(params).await.and_then(|x| Ok(serde_json::to_value(x)?)),
        Method::AddressInformation { params } => rpc.get_address_information(params).await.and_then(|x| Ok(serde_json::to_value(x)?)),
        Method::ExtendedAddressInformation { params } => rpc.get_extended_address_information(params).await.and_then(|x| Ok(serde_json::to_value(x)?)),
        Method::Transactions { params } => rpc.get_transactions(params).await.and_then(|x| Ok(serde_json::to_value(x)?)),
        Method::SendBoc { params } => rpc.send_boc(params).await.and_then(|x| Ok(serde_json::to_value(x)?))
    };

    Json(
        match result {
            Ok(v) => JsonResponse::new(payload.id, v),
            Err(e) => JsonResponse::error(payload.id, e)
        }
    )
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt::init();

    debug!("initialize ton client...");

    let ton = Ton::balanced("./liteserver_config.json").await?;

    debug!("initialized");

    let rpc = Arc::new(RpcServer {
        client: ton
    });

    let app = Router::new().route("/", post({
        let rpc = Arc::clone(&rpc);
        move |body| dispatch_method(body, Arc::clone(&rpc))
    }));

    axum::Server::bind(&"0.0.0.0:3030".parse().unwrap())
        .http1_keepalive(true)
        .tcp_nodelay(true)
        .tcp_keepalive(Some(Duration::from_secs(90)))
        .serve(app.into_make_service())
        .await
        .unwrap();

    Ok(())
}

fn base64_to_hex(b: &str) -> anyhow::Result<String> {
    let bytes = base64::decode(b)?;
    let hex = hex::encode(bytes);

    Ok(hex)
}

fn hex_to_base64(b: &str) -> anyhow::Result<String> {
    let bytes = hex::decode(b)?;
    let base64 = base64::encode(bytes);

    Ok(base64)
}
