use std::time::Duration;
use derive_new::new;
use serde::{Serialize, Serializer};
use serde::de::DeserializeOwned;
use serde_json::Value;
use ton_client_util::router::route::{Route, ToRoute};

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

impl<T> ToRoute for Forward<T> {
    fn to_route(&self) -> Route { self.route }
}

impl<T> Requestable for Forward<T> where T : Requestable {
    type Response = T::Response;
}

// TODO[akostylev0] reinvent that layer
#[derive(new, Clone)]
pub(crate) struct Specialized<T> {
    inner: T
}

impl<T> ToRoute for Specialized<T> where T : ToRoute {
    fn to_route(&self) -> Route {
        self.inner.to_route()
    }
}
