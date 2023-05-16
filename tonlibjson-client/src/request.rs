use std::time::Duration;
use async_trait::async_trait;
use derive_new::new;
use futures::TryFutureExt;
use uuid::Uuid;
use serde::{Serialize, Deserialize, Serializer};
use serde::de::DeserializeOwned;
use serde_json::Value;
use tower::{Service, ServiceExt};
use crate::balance::Route;
use crate::error::Error;

#[async_trait]
pub trait TypedCallable<S> : Sized + Send + 'static {
    type Response : DeserializeOwned;

    async fn typed_call(self, client: &mut S) -> anyhow::Result<Self::Response>;
}

#[async_trait]
impl<S, T, E: Into<Error>> TypedCallable<S> for T
    where T : Requestable + 'static,
          S : Service<T, Response=T::Response, Error=E> + Send,
          S::Future : Send + 'static,
          S::Error: Send {
    type Response = T::Response;

    async fn typed_call(self, client: &mut S) -> anyhow::Result<Self::Response> {
        Ok(client
            .ready()
            .map_err(Into::<Error>::into)
            .await?
            .call(self)
            .map_err(Into::<Error>::into)
            .await?)
    }
}

#[async_trait]
pub trait Requestable where Self : Serialize + Sized + Clone + Send + std::marker::Sync {
    type Response : DeserializeOwned + Send + std::marker::Sync + 'static;

    fn timeout(&self) -> Duration {
        Duration::from_secs(3)
    }
}

pub trait Routable {
    fn route(&self) -> Route;
}

#[derive(new, Debug, Clone)]
pub struct Forward<Req : Requestable + Clone> {
    req: Req,
    route: Route
}

impl<Req> Serialize for Forward<Req> where Req : Requestable {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error> where S: Serializer {
        self.req.serialize(serializer)
    }
}

impl<Req> Requestable for Forward<Req> where Req : Requestable {
    type Response = Req::Response;

    fn timeout(&self) -> Duration {
        self.req.timeout()
    }
}

impl<Req> Routable for Forward<Req> where Req : Requestable {
    fn route(&self) -> Route { self.route }
}

impl Requestable for Value {
    type Response = Value;
}

pub type RequestId = Uuid;

#[derive(Serialize, Clone)]
pub struct Request<T : Serialize + Clone> {
    #[serde(rename="@extra")]
    pub id: RequestId,

    #[serde(skip_serializing)]
    pub timeout: Duration,

    #[serde(flatten)]
    pub body: T
}

#[derive(Deserialize, Debug)]
pub struct Response {
    #[serde(rename="@extra")]
    pub id: RequestId,

    #[serde(flatten)]
    pub data: Value
}

#[cfg(test)]
mod tests {
    use std::str::FromStr;
    use std::time::Duration;
    use serde_json::json;
    use uuid::Uuid;
    use crate::request::{Request, Request};

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
