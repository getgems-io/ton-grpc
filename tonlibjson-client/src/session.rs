use std::future::Future;
use std::pin::Pin;
use std::task::{Context, Poll};
use derive_new::new;
use futures::FutureExt;
use serde_json::Value;
use tower::{Layer, Service};
use tower::load::{Load, PeakEwma};
use tower::load::peak_ewma::Cost;
use crate::{client::Client, request::Request};
use crate::balance::{BalanceRequest, Route};
use crate::block::{AccountAddress, SmcLoad, SmcMethodId, SmcRunGetMethod, SmcStack};
use crate::request::{Requestable, RequestableWrapper, Routable};
use crate::shared::{SharedLayer, SharedService};
use crate::request::Callable;

#[derive(new, Clone)]
pub enum SessionRequest {
    RunGetMethod { address: AccountAddress, method: SmcMethodId, stack: SmcStack },
    Atomic(Request)
}

impl Callable for SessionRequest {
    type Response = Value;
}

impl From<Request> for SessionRequest {
    fn from(req: Request) -> Self {
        SessionRequest::new_atomic(req)
    }
}

impl Routable for SessionRequest {
    fn route(&self) -> Route {
        match self {
            SessionRequest::Atomic(req) => req.body.route(),
            SessionRequest::RunGetMethod { address, .. } => Route::Latest { chain: address.chain_id() }
        }
    }
}

impl TryFrom<RequestableWrapper<SessionRequest>> for BalanceRequest {
    type Error = anyhow::Error;

    fn try_from(req: RequestableWrapper<SessionRequest>) -> Result<Self, Self::Error> {
        let req = req.inner;

        Ok(BalanceRequest::new(req.route(), req))
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
            }
        }
    }
}

impl SessionClient {
    fn run_get_method(&self, address: AccountAddress, method: SmcMethodId, stack: SmcStack) -> impl Future<Output=anyhow::Result<Value>> {
        let mut client = self.inner.clone();

        async move {
            let info = SmcLoad::new(address).call(&mut client).await?;

            SmcRunGetMethod::new(info.id, method, stack)
                .call(&mut client)
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
