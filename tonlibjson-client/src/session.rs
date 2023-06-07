use async_trait::async_trait;
use derive_new::new;
use futures::TryFutureExt;
use tower::{Service, ServiceExt};
use crate::balance::Route;
use crate::block::{AccountAddress, SmcLoad, SmcMethodId, SmcRunGetMethod, SmcStack};
use crate::error::Error;
use crate::request::{Requestable, Routable, Callable};

#[derive(new, Clone)]
pub struct RunGetMethod {
    address: AccountAddress,
    method: SmcMethodId,
    stack: SmcStack
}

#[async_trait]
impl<S, E: Into<Error> + Send> Callable<S> for RunGetMethod
    where S: Service<SmcLoad, Response=<SmcLoad as Requestable>::Response, Error=E>,
          <S as Service<SmcLoad>>::Future: Send,
          S: Service<SmcRunGetMethod, Response=<SmcRunGetMethod as Requestable>::Response, Error=E>,
          <S as Service<SmcRunGetMethod>>::Future: Send,
          S: Send {
    type Response = <SmcRunGetMethod as Requestable>::Response;

    async fn call(self, client: &mut S) -> anyhow::Result<Self::Response> {
        let info = ServiceExt::<SmcLoad>::ready(client)
            .map_err(Into::into)
            .await?
            .call(SmcLoad::new(self.address))
            .map_err(Into::into)
            .await?;

        let result = ServiceExt::<SmcRunGetMethod>::ready(client)
            .map_err(Into::into)
            .await?
            .call(SmcRunGetMethod::new(info.id, self.method, self.stack))
            .map_err(Into::into)
            .await?;

        Ok(result)
    }
}

impl Routable for RunGetMethod {
    fn route(&self) -> Route {
        Route::Latest { chain: self.address.chain_id() }
    }
}
