use crate::block::{
    AccountAddress, SmcBoxedMethodId, SmcLoad, SmcRunGetMethod, TvmBoxedStackEntry,
};
use crate::client::Client;
use crate::request::Requestable;
use derive_new::new;
use futures::future::BoxFuture;
use futures::FutureExt;
use futures::TryFutureExt;
use std::task::{Context, Poll};
use ton_client_util::router::route::{Route, ToRoute};
use tower::{Service, ServiceExt};

#[derive(new, Clone)]
pub struct RunGetMethod {
    address: AccountAddress,
    method: SmcBoxedMethodId,
    stack: Vec<TvmBoxedStackEntry>,
}

impl Service<RunGetMethod> for Client {
    type Response = <SmcRunGetMethod as Requestable>::Response;
    type Error = anyhow::Error;
    type Future = BoxFuture<'static, Result<Self::Response, Self::Error>>;

    fn poll_ready(&mut self, cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        <Self as Service<SmcLoad>>::poll_ready(self, cx)
    }

    fn call(&mut self, req: RunGetMethod) -> Self::Future {
        let clone = self.clone();

        self.call(SmcLoad::new(req.address))
            .and_then(move |info| {
                clone.oneshot(SmcRunGetMethod::new(info.id, req.method, req.stack))
            })
            .boxed()
    }
}

impl ToRoute for RunGetMethod {
    fn to_route(&self) -> Route {
        Route::Latest
    }
}
