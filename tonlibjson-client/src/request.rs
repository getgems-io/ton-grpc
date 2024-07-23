use derive_new::new;
use serde::de::DeserializeOwned;
use serde::{Serialize, Serializer};
use serde_json::Value;
use std::time::Duration;
use ton_client_util::router::route::{Route, ToRoute};
use ton_client_util::service::timeout::ToTimeout;

pub(crate) trait Requestable
where
    Self: Serialize,
{
    type Response: DeserializeOwned;
}

impl Requestable for Value {
    type Response = Value;
}

#[derive(Clone, Debug)]
pub(crate) struct Forward<T> {
    route: Route,
    inner: T,
}

impl<T> Forward<T> {
    pub(crate) fn new(route: Route, inner: T) -> Self {
        Self { route, inner }
    }
}

impl<T> Serialize for Forward<T>
where
    T: Serialize,
{
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        self.inner.serialize(serializer)
    }
}

impl<T> ToRoute for Forward<T> {
    fn to_route(&self) -> Route {
        self.route
    }
}

impl<T> Requestable for Forward<T>
where
    T: Requestable,
{
    type Response = T::Response;
}

impl<T> ToTimeout for Forward<T>
where
    T: ToTimeout,
{
    fn to_timeout(&self) -> Option<Duration> {
        self.inner.to_timeout()
    }
}

// TODO[akostylev0] reinvent that layer
#[derive(new, Clone)]
pub(crate) struct Specialized<T> {
    inner: T,
}

impl<T> ToRoute for Specialized<T>
where
    T: ToRoute,
{
    fn to_route(&self) -> Route {
        self.inner.to_route()
    }
}
