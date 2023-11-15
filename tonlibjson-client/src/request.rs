use std::future::Future;
use std::time::Duration;
use derive_new::new;
use uuid::Uuid;
use serde::{Serialize, Deserialize, Serializer};
use serde::de::DeserializeOwned;
use serde_json::Value;
use tower::{Service};
use crate::router::{Routable, Route};
use crate::error::Error;

pub trait Callable<S> : Sized + Send + 'static {
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

pub trait Requestable where Self : Serialize + Send + Sync {
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

pub type RequestId = Uuid;

#[derive(Serialize)]
pub(crate) struct Request<T : Serialize> {
    #[serde(rename="@extra")]
    pub id: RequestId,

    #[serde(skip_serializing)]
    pub timeout: Duration,

    #[serde(flatten)]
    pub body: T
}


// TODO[akostylev0] generic over request type
#[derive(Deserialize, Debug)]
pub(crate) struct Response {
    #[serde(rename="@extra")]
    pub id: RequestId,

    #[serde(flatten)]
    pub data: Value
}

// TODO[akostylev0] reinvent that layer
#[derive(new, Clone)]
pub struct Specialized<T> {
    inner: T
}

impl<T> Specialized<T> {
    pub fn inner(&self) -> &T {
        &self.inner
    }

    pub fn into_inner(self) -> T {
        self.inner
    }
}

impl<T> Routable for Specialized<T> where T : Routable {
    fn route(&self) -> Route {
        self.inner.route()
    }
}

#[cfg(test)]
mod tests {
    use std::str::FromStr;
    use std::time::Duration;
    use serde_json::json;
    use uuid::Uuid;
    use crate::request::Request;

    #[test]
    fn data_is_flatten() {
        let request = Request {
            id: Uuid::from_str("7431f198-7514-40ff-876c-3e8ee0a311ba").unwrap(),
            timeout: Duration::from_secs(3),
            body: json!({
                "data": "is flatten"
            })
        };

        assert_eq!(serde_json::to_string(&request).unwrap(), "{\"@extra\":\"7431f198-7514-40ff-876c-3e8ee0a311ba\",\"data\":\"is flatten\"}")
    }
}
