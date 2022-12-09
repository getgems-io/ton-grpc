use std::future::Future;
use std::io::Read;
use std::marker::PhantomData;
use std::pin::Pin;
use std::process::Output;
use std::sync::{Arc};
use tokio::sync::Mutex;
use std::task::{Context, Poll};
use std::time::Duration;
use anyhow::anyhow;
use futures::future::BoxFuture;
use futures::{FutureExt, StreamExt};
use futures::TryFutureExt;
use pin_project::pin_project;
use serde_json::Value;
use tower::{BoxError, Layer, Service, ServiceExt};
use tower::buffer::Buffer;
use tower::load::PeakEwma;
use tracing::debug;
use crate::{client::Client, request::Request};
use crate::block::{Sync, BlockHeader, BlockId, BlockIdExt, BlocksLookupBlock, GetBlockHeader, GetMasterchainInfo, MasterchainInfo, SmcInfo, SmcLoad, SmcMethodId, SmcRunGetMethod, SmcStack};
use crate::error::{ErrorLayer, ErrorService};

pub enum SessionRequest {
    RunGetMethod { address: String, method: String, stack: SmcStack },
    Atomic(Request),
    Synchronize {},
    FindFirsBlock {}
}

impl From<Request> for SessionRequest {
    fn from(req: Request) -> Self {
        SessionRequest::Atomic(req)
    }
}

#[derive(Clone)]
pub struct SessionClient {
    client: Client
}

impl SessionClient {
    pub fn new(client: Client) -> Self {
        Self { client }
    }
}

impl Service<SessionRequest> for SessionClient {
    type Response = Value;
    type Error = anyhow::Error;
    type Future = Pin<Box<dyn Future<Output = Result<Self::Response, Self::Error>> + Send>>;

    fn poll_ready(&mut self, cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        self.client.poll_ready(cx)
    }

    fn call(&mut self, req: SessionRequest) -> Self::Future {
        match req {
            SessionRequest::Atomic(req) => {
                self.client.call(req)
            },
            SessionRequest::RunGetMethod { address, method, stack} => {
                let mut this = self.clone();
                async move { this.run_get_method(address, method, stack).await }.boxed()
            },
            SessionRequest::Synchronize {} => {
                let mut this = self.clone();
                async move { this.synchronize().await }.boxed()
            },
            SessionRequest::FindFirsBlock {} => {
                let mut this = self.clone();
                async move { this.find_first_block().await }.boxed()
            }
        }
    }
}

impl SessionClient {
    fn run_get_method(&mut self, address: String, method: String, stack: SmcStack) -> BoxFuture<anyhow::Result<Value>> {
        let req = SmcLoad::new(address);

        async move {
            let resp = self.client
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

            self.client
                .ready()
                .await?
                .call(Request::new(&req)?)
                .await
        }.boxed()
    }

    pub fn synchronize(&mut self) -> BoxFuture<anyhow::Result<Value>> {
        async move {
            let request = Request::with_timeout(Sync::default(), Duration::from_secs(60))?;

            let response = self.client
                .ready()
                .await?
                .call(request)
                .await?;

            let block = serde_json::from_value::<BlockIdExt>(response)?;

            self.block_header(block).await
        }.boxed()
    }

    pub fn find_first_block(&mut self) -> BoxFuture<anyhow::Result<Value>> {
        async move {
            let masterchain_info = self.client
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
            let mut block = self.client
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

                block = self.client
                    .ready()
                    .await?
                    .call(Request::new(request)?)
                    .await;
            }

            let block: BlockIdExt = serde_json::from_value(block?)?;

            self.block_header(block).await
        }.boxed()
    }

    pub async fn block_header(&mut self, block: BlockIdExt) -> anyhow::Result<Value> {
        let request = Request::new(GetBlockHeader::new(block.clone()))?;

        let response = self.client
            .ready()
            .await?
            .call(request)
            .await?;

        // let header = serde_json::from_value::<BlockHeader>(response)?;

        Ok(response)
    }
}
