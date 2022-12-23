use std::borrow::BorrowMut;
use std::future::{Future, join};
use std::pin::Pin;
use std::task::{Context, Poll};
use std::time::Duration;
use anyhow::anyhow;
use futures::{FutureExt, TryFutureExt};
use serde_json::Value;
use tower::{Layer, Service, ServiceExt};
use tower::load::{Load, PeakEwma};
use tower::load::peak_ewma::Cost;
use tracing::debug;
use crate::{client::Client, request::Request};
use crate::block::{Sync, BlockId, BlockIdExt, BlocksLookupBlock, GetBlockHeader, GetMasterchainInfo, MasterchainInfo, SmcInfo, SmcLoad, SmcMethodId, SmcRunGetMethod, SmcStack, BlocksGetShards, ShardsResponse};
use crate::request::Requestable;
use crate::shared::{SharedLayer, SharedService};

#[derive(Clone)]
pub enum SessionRequest {
    RunGetMethod { address: String, method: String, stack: SmcStack },
    Atomic(Request),
    GetMasterchainInfo {},
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
            SessionRequest::FindFirstBlock { chain_id } => {
                self.find_first_block(chain_id).boxed()
            },
            SessionRequest::GetMasterchainInfo {} => self.get_masterchain_info().boxed()
        }
    }
}

impl SessionClient {
    fn get_masterchain_info(&self) -> impl Future<Output=anyhow::Result<Value>> {
        let mut client = self.inner.clone();

        async move {
            let response = client
                .ready()
                .await?
                .call(Request::new(GetMasterchainInfo::default())?)
                .await?;

            Ok(response)
        }
    }

    #[allow(dead_code)]
    fn current_block(&self) -> impl Future<Output=anyhow::Result<Value>> {
        let mut client = self.inner.clone();

        async move {
            let response = client
                .ready()
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
}

async fn find_last_block(client: &mut SharedService<PeakEwma<Client>>, workchain: i64) -> anyhow::Result<BlockIdExt> {
    let masterchain_info = client
        .ready()
        .await?
        .call(GetMasterchainInfo::default().into_request()?)
        .await?;
    let masterchain_info: MasterchainInfo = serde_json::from_value(masterchain_info)?;

    match workchain {
        -1 => {
            Ok(masterchain_info.last)
        },
        chain_id => {
            let request = BlocksGetShards::new(masterchain_info.last).into_request()?;

            let shards = client
                .ready()
                .await?
                .call(request)
                .await?;
            let shards: ShardsResponse = serde_json::from_value(shards)?;

            for shard in shards.shards {
                if shard.workchain == chain_id {
                    return Ok(shard)
                }
            }

            Err(anyhow!("chain not found"))
        }
    }
}


impl Load for SessionClient {
    type Metric = Cost;

    fn load(&self) -> Self::Metric {
        self.inner.load()
    }
}
