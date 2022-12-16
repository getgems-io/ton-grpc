use std::future::Future;
use std::pin::Pin;
use std::task::{Context, Poll};
use std::time::Duration;
use futures::FutureExt;
use serde_json::Value;
use tower::{Layer, Service, ServiceExt};
use tower::load::{Load, PeakEwma};
use tower::load::peak_ewma::Cost;
use tracing::debug;
use crate::{client::Client, request::Request};
use crate::block::{Sync, BlockId, BlockIdExt, BlocksLookupBlock, GetBlockHeader, GetMasterchainInfo, MasterchainInfo, SmcInfo, SmcLoad, SmcMethodId, SmcRunGetMethod, SmcStack};
use crate::shared::{SharedLayer, SharedService};

#[derive(Clone)]
pub enum SessionRequest {
    RunGetMethod { address: String, method: String, stack: SmcStack },
    Atomic(Request),
    Synchronize {},
    FindFirstBlock {},
    CurrentBlock {}
}

impl From<Request> for SessionRequest {
    fn from(req: Request) -> Self {
        SessionRequest::Atomic(req)
    }
}

#[derive(Clone)]
pub struct SessionClient {
    inner: SharedService<PeakEwma<Client>>
}

impl SessionClient {
    pub fn new(client: PeakEwma<Client>) -> Self {
        Self { inner: SharedLayer::default().layer(client) }
    }
}

impl Service<SessionRequest> for SessionClient {
    type Response = Value;
    type Error = anyhow::Error;
    type Future = Pin<Box<dyn Future<Output = Result<Self::Response, Self::Error>> + Send>>;

    fn poll_ready(&mut self, cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        self.inner.poll_ready(cx)
    }

    fn call(&mut self, req: SessionRequest) -> Self::Future {
        match req {
            SessionRequest::Atomic(req) => {
                self.inner.call(req).boxed()
            },
            SessionRequest::RunGetMethod { address, method, stack} => {
                self.run_get_method(address, method, stack).boxed()
            },
            SessionRequest::Synchronize {} => {
                self.synchronize().boxed()
            },
            SessionRequest::FindFirstBlock {} => {
                self.find_first_block().boxed()
            },
            SessionRequest::CurrentBlock {} => {
                self.current_block().boxed()
            }
        }
    }
}

impl SessionClient {
    fn current_block(&self) -> impl Future<Output=anyhow::Result<Value>> {
        let mut client = self.inner.clone();

        async move {
            let response = client.
                ready()
                .await?
                .call(Request::new(GetMasterchainInfo::default())?)
                .await?;

            let block = serde_json::from_value::<MasterchainInfo>(response)?;

            let request = Request::new(GetBlockHeader::new(block.last))?;
            let response = client
                .ready()
                .await?
                .call(request)
                .await?;

            Ok(response)
        }
    }

    fn run_get_method(&self, address: String, method: String, stack: SmcStack) -> impl Future<Output=anyhow::Result<Value>> {
        let mut client = self.inner.clone();
        let req = SmcLoad::new(address);

        async move {
            let resp = client
                .ready()
                .await?
                .call(Request::new(&req)?)
                .await?;

            let info = serde_json::from_value::<SmcInfo>(resp)?;

            let req = SmcRunGetMethod::new(
                info.id,
                SmcMethodId::Name {name: method},
                stack
            );

            client
                .ready()
                .await?
                .call(Request::new(&req)?)
                .await
        }
    }

    pub fn synchronize(&self) -> impl Future<Output=anyhow::Result<Value>> {
        let mut client = self.inner.clone();

        async move {
            let request = Request::with_timeout(Sync::default(), Duration::from_secs(60))?;

            let response = client
                .ready()
                .await?
                .call(request)
                .await?;

            let block = serde_json::from_value::<BlockIdExt>(response)?;

            let request = Request::new(GetBlockHeader::new(block))?;
            let response = client
                .ready()
                .await?
                .call(request)
                .await?;

            Ok(response)
        }
    }

    pub fn find_first_block(&self) -> impl Future<Output=anyhow::Result<Value>> {
        let mut client = self.inner.clone();

        async move {
            let masterchain_info = client
                .ready()
                .await?
                .call(Request::new(GetMasterchainInfo {})?)
                .await?;
            let masterchain_info: MasterchainInfo = serde_json::from_value(masterchain_info)?;

            let length = masterchain_info.last.seqno;
            let mut cur = length / 2;
            let mut rhs = length;
            let mut lhs = masterchain_info.init.seqno;

            let workchain = masterchain_info.last.workchain;
            let shard = masterchain_info.last.shard;

            let request = BlocksLookupBlock::new(BlockId {
                workchain,
                shard: shard.clone(),
                seqno: cur
            }, 0, 0);
            let mut block = client
                .ready()
                .await?
                .call(Request::new(request)?)
                .await;

            while lhs < rhs {
                // TODO[akostylev0] specify error
                if block.is_err() {
                    lhs = cur + 1;
                } else {
                    rhs = cur;
                }

                cur = (lhs + rhs) / 2;

                debug!("lhs: {}, rhs: {}, cur: {}", lhs, rhs, cur);

                let request = BlocksLookupBlock::new(BlockId {
                    workchain,
                    shard: shard.clone(),
                    seqno: cur
                }, 0, 0);

                block = client
                    .ready()
                    .await?
                    .call(Request::new(request)?)
                    .await;
            }

            let block: BlockIdExt = serde_json::from_value(block?)?;

            let request = Request::new(GetBlockHeader::new(block))?;
            let response = client
                .ready()
                .await?
                .call(request)
                .await?;

            Ok(response)
        }
    }
}


impl Load for SessionClient {
    type Metric = Cost;

    fn load(&self) -> Self::Metric {
        self.inner.load()
    }
}
