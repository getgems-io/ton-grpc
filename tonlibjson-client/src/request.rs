use std::future::Future;
use std::time::Duration;
use derive_new::new;
use serde::{Serialize, Serializer};
use serde::de::DeserializeOwned;
use serde_json::Value;
use tower::{Service};
use ton_client_utils::router::Route;
use crate::router::Routable;
use crate::error::Error;

pub(crate) trait Callable<S> : Sized + Send + 'static {
    type Response : DeserializeOwned;
    type Error: Into<Error>;
    type Future : Future<Output=Result<Self::Response, Self::Error>> + Send;

    fn call(self, client: &mut S) -> Self::Future;
}

impl<S, T, E: Into<Error>> Callable<S> for T
    where T : Requestable + 'static,
          S : Service<T, Response=T::Response, Error=E> + Send,
          S::Future : Send + 'static,
          S::Error: Send {
    type Response = T::Response;
    type Error = S::Error;
    type Future = S::Future;

    fn call(self, client: &mut S) -> Self::Future {
        client.call(self)
    }
}

pub(crate) trait Requestable where Self : Serialize + Send + Sync {
    type Response : DeserializeOwned + Send + Sync + 'static;

    fn timeout(&self) -> Duration {
        Duration::from_secs(3)
    }
}

impl Requestable for Value {
    type Response = Value;
}

#[derive(Clone, Debug)]
pub(crate) struct Forward<T> {
    route: Route,
    inner: T
}

impl<T> Forward<T> {
    pub(crate) fn new(route: Route, inner: T) -> Self {
        Self { route, inner }
    }
}

impl<T: Serialize> Serialize for Forward<T> {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error> where S: Serializer {
        self.inner.serialize(serializer)
    }
}

impl<T> Routable for Forward<T> {
    fn route(&self) -> Route { self.route }
}

impl<T> Requestable for Forward<T> where T : Requestable {
    type Response = T::Response;
}

// TODO[akostylev0] reinvent that layer
#[derive(new, Clone)]
pub(crate) struct Specialized<T> {
    inner: T
}

impl<T> Routable for Specialized<T> where T : Routable {
    fn route(&self) -> Route {
        self.inner.route()
    }
}
