use derive_new::new;
use futures::future::BoxFuture;
use futures::TryFutureExt;
use futures::FutureExt;
use tower::{Service, ServiceExt};
use crate::router::Route;
use crate::block::{AccountAddress, SmcBoxedMethodId, SmcLoad, SmcRunGetMethod, TvmBoxedStackEntry};
use crate::error::Error;
use crate::request::{Requestable, Callable};
use crate::router::Routable;

#[derive(new, Clone)]
pub struct RunGetMethod {
    address: AccountAddress,
    method: SmcBoxedMethodId,
    stack: Vec<TvmBoxedStackEntry>
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
    fn route(&self) -> Route { Route::Latest }
}
