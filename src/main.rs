use jsonrpc_core::{BoxFuture, Params};
use crate::tonlib::{AsyncClient, base64_to_hex, BlockIdExt, ClientBuilder, ShortTxId, TlBlock};
use jsonrpc_http_server::jsonrpc_core::IoHandler;
use jsonrpc_http_server::tokio::runtime::Runtime;
use jsonrpc_http_server::{ServerBuilder};
use jsonrpc_derive::rpc;
use jsonrpc_core::{Result, Error};
use serde_json::{json, Value};
use serde::Deserialize;
use futures::StreamExt;

#[macro_use]
extern crate lazy_static;

mod tonlib;

lazy_static! {
    static ref TON: AsyncClient = {
        let client = Runtime::new().unwrap().block_on(async {
            ClientBuilder::from_file("./liteserver_config.json")
                .unwrap()
                .disable_logging()
                .build()
                .await
                .unwrap()
        });

        client
    };
}

#[derive(Deserialize, Debug)]
struct LookupBlockParams {
    workchain: i64,
    shard: String,
    seqno: Option<u64>,
    lt: Option<i64>,
    unixtime: Option<u64>
}

#[derive(Deserialize, Debug)]
struct ShardsParams {
    seqno: u64
}

#[derive(Deserialize, Debug)]
struct BlockHeaderParams {
    workchain: i64,
    shard: String,
    seqno: u64,
    root_hash: Option<String>,
    file_hash: Option<String>
}

#[derive(Deserialize, Debug)]
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

#[rpc(server)]
pub trait Rpc {
    #[rpc(name = "getMasterchainInfo")]
    fn master_chain_info(&self) -> BoxFuture<Result<Value>>;

    #[rpc(name = "lookupBlock", raw_params)]
    fn lookup_block(&self, params: Params) -> BoxFuture<Result<Value>>;

    #[rpc(name = "shards", raw_params)]
    fn shards(&self, params: Params) -> BoxFuture<Result<Value>>;

    #[rpc(name = "getBlockHeader", raw_params)]
    fn get_block_header(&self, params: Params) -> BoxFuture<Result<Value>>;

    #[rpc(name = "getBlockTransactions", raw_params)]
    fn get_block_transactions(&self, params: Params) -> BoxFuture<Result<Value>>;
}

struct RpcImpl;

impl Rpc for RpcImpl {
    fn master_chain_info(&self) -> BoxFuture<Result<Value>> {
        Box::pin(async {
            jsonrpc_error(TON.get_masterchain_info().await)
        })
    }

    fn lookup_block(&self, params: Params) -> BoxFuture<Result<Value>> {
        Box::pin(async move {
            let params = params.parse::<LookupBlockParams>()?;

            let workchain = params.workchain;
            let shard = params.shard.parse::<i64>().map_err(|_| Error::invalid_params("invalid shard"))?;
            match (params.seqno, params.lt, params.unixtime) {
                (Some(seqno), None, None) if seqno > 0 => jsonrpc_error(TON.look_up_block_by_seqno(workchain, shard, seqno).await),
                (None, Some(lt), None) if lt > 0 => jsonrpc_error(TON.look_up_block_by_lt(workchain, shard, lt).await),
                (None, None, Some(_)) => Err(Error::invalid_params("unixtime is not supported")),
                _ => Err(Error::invalid_params("seqno or lt or unixtime must be provided"))
            }
        })
    }

    fn shards(&self, params: Params) -> BoxFuture<Result<Value>> {
        Box::pin(async move {
            let params = params.parse::<ShardsParams>()?;

            jsonrpc_error(TON.get_shards(params.seqno).await)
        })
    }

    fn get_block_header(&self, params: Params) -> BoxFuture<Result<Value>> {
        Box::pin(async move {
            let params = params.parse::<BlockHeaderParams>()?;
            let shard = params.shard.parse::<i64>().map_err(|_| Error::invalid_params("invalid shard"))?;


            jsonrpc_error(TON.get_block_header(
                params.workchain,
                shard,
                params.seqno
            ).await)
        })
    }

    fn get_block_transactions(&self, params: Params) -> BoxFuture<jsonrpc_core::Result<Value>> {
        Box::pin(async move {
            let params = params.parse::<BlockTransactionsParams>()?;
            let shard = params.shard.parse::<i64>().map_err(|_| Error::invalid_params("invalid shard"))?;
            let count = params.count.unwrap_or(200);

            let block_json = TON
                .look_up_block_by_seqno(params.workchain, shard, params.seqno)
                .await.map_err(|_| Error::internal_error())?;

            let block = serde_json::from_value::<BlockIdExt>(block_json)
                .map_err(|_| Error::internal_error())?;

            let stream = TON.get_tx_stream(block.clone()).await.map_err(|_| Error::internal_error())?;
            let tx: Vec<TlBlock> = stream
                .map(|tx: ShortTxId| TlBlock::ShortTxId(ShortTxId {
                    account: format!("{}:{}", block.workchain, base64_to_hex(&tx.account).unwrap()),
                    hash: tx.hash,
                    lt: tx.lt,
                    mode: tx.mode
                }))
                .collect()
                .await;


            Ok(json!({
                "@type": "blocks.transactions",
                "id": TlBlock::BlockIdExt(block),
                "incomplete": false,
                "req_count": count,
                "transactions": &tx
            }))
        })
    }
}

fn jsonrpc_error<T>(r: anyhow::Result<T>) -> jsonrpc_core::Result<T> {
    r.map_err(|_| jsonrpc_core::Error::internal_error())
}

fn main() {
    let mut rt = Runtime::new().unwrap();
    println!("{:?}", rt.block_on(TON.get_masterchain_info()));

    let mut io = IoHandler::new();
    io.extend_with(RpcImpl.to_delegate());

    let server = ServerBuilder::new(io)
        .event_loop_executor(rt.handle().clone())
        .threads(1)
        .start_http(&"127.0.0.1:3030".parse().unwrap())
        .unwrap();

    server.wait()
}

// #[tokio::main(flavor = "multi_thread")]
// async fn main() -> Result<(), Box<dyn std::error::Error>> {
//     let client = Arc::new(
//         ClientBuilder::from_file("./liteserver_config.json")?
//             .disable_logging()
//             .build()
//             .await?,
//     );
//
//     let _ = tokio::task::spawn_blocking({
//         let client = client.clone();
//
//         move || {
//             let mut io = jsonrpc_core::IoHandler::new();
//             let client = client.clone();
//             io.add_method("getMasterchainInfo", {
//                 move |_params: Params| async {
//                     client
//                         .get_masterchain_info()
//                         .await
//                         .map_err(|e| jsonrpc_core::Error::new(ServerError(1)))
//                 }
//             });
//
//             let server = ServerBuilder::new(io)
//                 .threads(3)
//                 .start_http(&"127.0.0.1:3030".parse().unwrap())
//                 .unwrap();
//
//             server.wait();
//         }
//     })
//     .await;
//
//     Ok(())
// }
