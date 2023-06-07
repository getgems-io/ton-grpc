use derive_new::new;
use futures::future::BoxFuture;
use futures::TryFutureExt;
use futures::FutureExt;
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

impl<S, E: Into<Error> + Send + 'static> Callable<S> for RunGetMethod
    where S: Service<SmcLoad, Response=<SmcLoad as Requestable>::Response, Error=E>,
          <S as Service<SmcLoad>>::Future: Send,
          S: Service<SmcRunGetMethod, Response=<SmcRunGetMethod as Requestable>::Response, Error=E>,
          <S as Service<SmcRunGetMethod>>::Future: Send,
          S: Send + Clone + 'static {
    type Response = <SmcRunGetMethod as Requestable>::Response;
    type Error = Error;
    type Future = BoxFuture<'static, Result<Self::Response, Self::Error>>;

    fn call(self, client: &mut S) -> Self::Future {
        let clone = client.clone();

        client.call(SmcLoad::new(self.address))
            .map_err(Into::into)
            .and_then(move |info| {
            clone
                .oneshot(SmcRunGetMethod::new(info.id, self.method, self.stack))
                .map_err(Into::into)
        }).boxed()
    }
}

impl Routable for RunGetMethod {
    fn route(&self) -> Route {
        Route::Latest { chain: self.address.chain_id() }
    }
}
