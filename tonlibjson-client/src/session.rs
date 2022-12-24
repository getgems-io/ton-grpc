use std::future::Future;
use std::pin::Pin;
use std::task::{Context, Poll};
use futures::FutureExt;
use serde_json::Value;
use tower::{Layer, Service};
use tower::load::{Load, PeakEwma};
use tower::load::peak_ewma::Cost;
use crate::{client::Client, request::Request};
use crate::block::{AccountAddress, GetMasterchainInfo, SmcLoad, SmcMethodId, SmcRunGetMethod, SmcStack};
use crate::request::{Requestable, RequestableWrapper};
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

impl<T> TryFrom<RequestableWrapper<T>> for SessionRequest where T : Requestable {
    type Error = anyhow::Error;

    fn try_from(req: RequestableWrapper<T>) -> Result<Self, Self::Error> {
        req.inner.into_request().map(SessionRequest::Atomic)
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
            SessionRequest::GetMasterchainInfo {} => self.get_masterchain_info().boxed()
        }
    }
}

impl SessionClient {
    fn get_masterchain_info(&self) -> impl Future<Output=anyhow::Result<Value>> {
        let mut client = self.inner.clone();

        async move {
            GetMasterchainInfo::default().call_value(&mut client).await
        }
    }

    fn run_get_method(&self, address: String, method: String, stack: SmcStack) -> impl Future<Output=anyhow::Result<Value>> {
        let mut client = self.inner.clone();

        async move {
            let address = AccountAddress::new(address)?;
            let info = SmcLoad::new(address).call(&mut client).await?;

            SmcRunGetMethod::new(info.id, SmcMethodId::new_name(method), stack)
                .call_value(&mut client)
                .await
        }
    }
}


impl Load for SessionClient {
    type Metric = Cost;

    fn load(&self) -> Self::Metric {
        self.inner.load()
    }
}
