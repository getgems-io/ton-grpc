use std::future::Future;
use std::pin::Pin;
use std::task::{Context, Poll};
use async_trait::async_trait;
use derive_new::new;
use futures::{FutureExt, TryFutureExt};
use tower::{Layer, Service, ServiceExt};
use tower::load::{Load, PeakEwma};
use tower::load::peak_ewma::Cost;
use crate::{client::Client};
use crate::balance::Route;
use crate::block::{AccountAddress, GetMasterchainInfo, SmcLoad, SmcMethodId, SmcRunGetMethod, SmcStack};
use crate::error::Error;
use crate::request::{Requestable, Routable, TypedCallable};
use crate::shared::{SharedLayer, SharedService};

#[derive(new, Clone)]
pub struct RunGetMethod {
    address: AccountAddress,
    method: SmcMethodId,
    stack: SmcStack
}

#[async_trait]
impl<S, E: Into<Error> + Send> TypedCallable<S> for RunGetMethod
    where S: Service<SmcLoad, Response=<SmcLoad as Requestable>::Response, Error=E>,
          <S as Service<SmcLoad>>::Future: Send,
          S: Service<SmcRunGetMethod, Response=<SmcRunGetMethod as Requestable>::Response, Error=E>,
          <S as Service<SmcRunGetMethod>>::Future: Send,
          S: Send {
    type Response = <SmcRunGetMethod as Requestable>::Response;

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
        <SharedService<PeakEwma<Client>> as Service<GetMasterchainInfo>>::poll_ready(&mut self.inner, cx)
    }

    fn call(&mut self, req: T) -> Self::Future {
        let mut client = self.inner.clone();

        async move {
            req.typed_call(&mut client).await
        }.boxed()
    }
}

impl Load for SessionClient {
    type Metric = Cost;

    fn load(&self) -> Self::Metric {
        self.inner.load()
    }
}
