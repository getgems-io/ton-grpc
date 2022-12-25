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
use crate::balance::Route;
use crate::block::{AccountAddress, SmcLoad, SmcMethodId, SmcRunGetMethod, SmcStack};
use crate::request::{CallableWrapper, Routable};
use crate::shared::{SharedLayer, SharedService};
use crate::request::Callable;

#[derive(new, Clone)]
pub enum SessionRequest {
    RunGetMethod { address: AccountAddress, method: SmcMethodId, stack: SmcStack },
    Atomic(Request)
}


#[derive(new)]
pub struct RunGetMethod {
    address: AccountAddress,
    method: SmcMethodId,
    stack: SmcStack
}

impl Callable for RunGetMethod {
    type Response = Value;
}

impl Routable for RunGetMethod {
    fn route(&self) -> Route {
        Route::Latest { chain: self.address.chain_id() }
    }
}

impl From<CallableWrapper<RunGetMethod>> for SessionRequest {
    fn from(value: CallableWrapper<RunGetMethod>) -> Self {
        let value = value.inner;

        SessionRequest::RunGetMethod { address: value.address, method: value.method, stack: value.stack }
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
