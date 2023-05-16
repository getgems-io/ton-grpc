use std::future::Future;
use std::pin::Pin;
use std::task::{Context, Poll};
use async_trait::async_trait;
use derive_new::new;
use futures::{FutureExt, TryFutureExt};
use serde_json::Value;
use tower::{Layer, Service, ServiceExt};
use tower::load::{Load, PeakEwma};
use tower::load::peak_ewma::Cost;
use crate::{client::Client, request::Request};
use crate::balance::Route;
use crate::block::{AccountAddress, SmcLoad, SmcMethodId, SmcRunGetMethod, SmcStack};
use crate::error::Error;
use crate::request::{CallableWrapper, Routable, TypedCallable};
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

#[async_trait]
impl<S, E: Into<Error> + Send> TypedCallable<S> for RunGetMethod
    where S: Service<SmcLoad, Response=<SmcLoad as Callable>::Response, Error=E>,
          <S as Service<SmcLoad>>::Future: Send,
          S: Service<SmcRunGetMethod, Response=<SmcRunGetMethod as Callable>::Response, Error=E>,
          <S as Service<SmcRunGetMethod>>::Future: Send,
          S: Send {
    type Response = <SmcRunGetMethod as Callable>::Response;

    async fn typed_call(self, client: &mut S) -> anyhow::Result<Self::Response> {
        let info = ServiceExt::<SmcLoad>::ready(client)
            .map_err(Into::into)
            .await?
            .call(SmcLoad::new(self.address))
            .map_err(Into::into)
            .await?;

        Ok(ServiceExt::<SmcRunGetMethod>::ready(client)
            .map_err(Into::into)
            .await?
            .call(SmcRunGetMethod::new(info.id, self.method, self.stack))
            .map_err(Into::into)
            .await?)
    }
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

impl<T> Service<T> for SessionClient where T: TypedCallable<SharedService<PeakEwma<Client>>> {
    type Response = T::Response;
    type Error = anyhow::Error;
    type Future = Pin<Box<dyn Future<Output = Result<Self::Response, Self::Error>> + Send>>;

    fn poll_ready(&mut self, cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        <SharedService<PeakEwma<Client>> as Service<Request>>::poll_ready(&mut self.inner, cx)
    }

    fn call(&mut self, req: T) -> Self::Future {
        let mut client = self.inner.clone();

        async move{
            req.typed_call(&mut client).await
        }.boxed()
    }
}

impl Service<SessionRequest> for SessionClient {
    type Response = Value;
    type Error = anyhow::Error;
    type Future = Pin<Box<dyn Future<Output = Result<Self::Response, Self::Error>> + Send>>;

    fn poll_ready(&mut self, cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        <SharedService<PeakEwma<Client>> as Service<Request>>::poll_ready(&mut self.inner, cx)
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
            let info = ServiceExt::<SmcLoad>::ready(&mut client).await?
                .call(SmcLoad::new(address)).await?;

            ServiceExt::<SmcRunGetMethod>::ready(&mut client).await?
                .call(SmcRunGetMethod::new(info.id, method, stack)).await
        }
    }
}


impl Load for SessionClient {
    type Metric = Cost;

    fn load(&self) -> Self::Metric {
        self.inner.load()
    }
}
