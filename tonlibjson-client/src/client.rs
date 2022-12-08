use std::future::Future;
use std::pin::Pin;
use std::sync::{Arc, mpsc, Mutex, RwLock};
use std::sync::mpsc::TryRecvError;
use std::task::{Context, Poll};
use std::time::Duration;
use anyhow::anyhow;
use serde_json::{json, Value};
use dashmap::DashMap;
use tower::{Service, ServiceExt};
use tracing::{debug, info, warn};
use crate::block::{AccountTransactionId, BlockHeader, BlockId, BlockIdExt, BlocksLookupBlock, GetBlockHeader, GetMasterchainInfo, MasterchainInfo, Sync, TonError};
use crate::request::{Request, RequestId, Response};

#[derive(Debug, Clone)]
pub struct Client {
    client: Arc<tonlibjson_sys::Client>,
    responses: Arc<DashMap<RequestId, tokio::sync::oneshot::Sender<Response>>>,
    _stop_signal: Arc<Mutex<Stop>>
}

impl Client {
    pub fn new() -> Self {
        let client = Arc::new(tonlibjson_sys::Client::new());
        let client_recv = client.clone();

        let responses: Arc<DashMap<RequestId, tokio::sync::oneshot::Sender<Response>>> =
            Arc::new(DashMap::new());
        let responses_rcv = Arc::clone(&responses);
        let (stop_signal, stop_receiver) = mpsc::channel();

        let _ = tokio::task::spawn_blocking(move || {
            let timeout = Duration::from_secs(20);

            loop {
                match stop_receiver.try_recv() {
                    Ok(_) | Err(TryRecvError::Disconnected) => {
                        tracing::warn!("Stop thread");
                        return
                    },
                    Err(TryRecvError::Empty) => {
                        if let Ok(packet) = client_recv.receive(timeout) {
                            if let Ok(response) = serde_json::from_str::<Response>(packet) {
                                if let Some((_, sender)) = responses_rcv.remove(&response.id) {
                                    let _ = sender.send(response);
                                }
                            } else if packet.contains("syncState") {
                                tracing::error!("Sync state: {}", packet.to_string());
                                if packet.contains("syncStateDone") {
                                    tracing::info!("syncState: {:#?}", packet);
                                }
                            } else {
                                tracing::warn!("Unexpected response {:?}", packet.to_string())
                            }
                        }
                    }
                }
            }
        });

        Client {
            client,
            responses,
            _stop_signal: Arc::new(Mutex::new(Stop::new(stop_signal)))
        }
    }

    pub async fn find_first_block(&mut self) -> anyhow::Result<BlockHeader> {
        let masterchain_info: MasterchainInfo = serde_json::from_value(self.ready().await?
            .call(Request::new(GetMasterchainInfo {})?).await?)?;

        let length = masterchain_info.last.seqno;
        let mut cur = length / 2;
        let mut rhs = length;
        let mut lhs = masterchain_info.init.seqno;

        let workchain = masterchain_info.last.workchain;
        let shard = masterchain_info.last.shard;

        let request = BlocksLookupBlock::new(&BlockId {
            workchain,
            shard: shard.clone(),
            seqno: cur
        }, 0, 0);
        let mut block = self.ready().await?.call(Request::new(request)?).await;

        while lhs < rhs {
            // TODO[akostylev0] specify error
            if block.is_err() {
                lhs = cur + 1;
            } else {
                let b = serde_json::from_value::<BlockIdExt>(block.unwrap()).unwrap();

                let request = json!({
                    "@type": "blocks.getTransactions",
                    "id": b,
                    "mode": 7 + 128,
                    "count": 1,
                    "after": AccountTransactionId::default(),
                });

                let header = self
                    .ready()
                    .await?
                    .call(Request::new(request)?)
                    .await;

                if header.is_err() {
                    lhs = cur + 1;
                } else {
                    rhs = cur;
                }
            }

            cur = (lhs + rhs) / 2;

            debug!("lhs: {}, rhs: {}, cur: {}", lhs, rhs, cur);

            let request = BlocksLookupBlock::new(&BlockId {
                workchain,
                shard: shard.clone(),
                seqno: cur
            }, 0, 0);

            block = self
                .ready()
                .await?
                .call(Request::new(request)?)
                .await;
        }

        let block: BlockIdExt = serde_json::from_value(block?)?;

        self.block_header(block).await
    }

    pub async fn synchronize(&mut self) -> anyhow::Result<BlockHeader> {
        let request = Request::with_timeout(Sync::default(), Duration::from_secs(60))?;

        let response = self.ready()
            .await?
            .call(request)
            .await?;

        let block = serde_json::from_value::<BlockIdExt>(response)?;

        self.block_header(block).await
    }

    pub async fn block_header(&mut self, block: BlockIdExt) -> anyhow::Result<BlockHeader> {
        let request = Request::new(GetBlockHeader::new(block.clone()))?;

        let response = self.ready()
            .await?
            .call(request)
            .await?;

        let header = serde_json::from_value::<BlockHeader>(response)?;

        Ok(header)
    }
}

impl Service<Request> for Client {
    type Response = Value;
    type Error = anyhow::Error;
    type Future = Pin<Box<dyn Future<Output = Result<Self::Response, Self::Error>> + Send>>;

    fn poll_ready(&mut self, cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        Poll::Ready(Ok(()))
    }

    fn call(&mut self, req: Request) -> Self::Future {
        let to_string = serde_json::to_string(&req);
        let Ok(query) = to_string else {
            return Box::pin(futures::future::ready(Err(anyhow!(to_string.unwrap_err()))));
        };

        let requests = Arc::clone(&self.responses);
        let (tx, rx) = tokio::sync::oneshot::channel::<Response>();
        requests.insert(req.id, tx);

        let _ = self.client.send(&query);

        Box::pin(async move {
            let result = tokio::time::timeout(req.timeout, rx).await;
            requests.remove(&req.id);

            let response = result??;

            // TODO[akostylev0] refac
            if response.data["@type"] == "error" {
                warn!("Error occurred: {:?}", &response.data);
                let error = serde_json::from_value::<TonError>(response.data)?;

                return Err(anyhow!(error))
            }

            Ok(response.data)
        })
    }
}

impl Drop for Client {
    fn drop(&mut self) {
        warn!("Drop client");
    }
}

#[derive(Debug)]
struct Stop {
    sender: mpsc::Sender<()>
}

impl Stop {
    fn new(sender: mpsc::Sender<()>) -> Self {
        Self {
            sender
        }
    }
}

impl Drop for Stop {
    fn drop(&mut self) {
        let _ = self.sender.send(());
    }
}
