use std::time::Duration;
use async_trait::async_trait;
use derive_new::new;
use uuid::Uuid;
use serde::{Serialize, Deserialize, Serializer};
use serde::de::DeserializeOwned;
use serde_json::Value;
use tower::{Service, ServiceExt};
use crate::balance::Route;

#[async_trait]
pub trait Callable : Sized {
    type Response : DeserializeOwned;
    
    async fn call<Req, S : Service<Req, Response = Value, Error = anyhow::Error>>(self, client: &mut S) -> Result<Self::Response, S::Error>
        where Req: Send,
              S : Send,
              S::Future : Send,
              RequestableWrapper<Self> : TryInto<Req, Error = S::Error>
    {
        let request = RequestableWrapper::new(self).try_into()?;

        let json = client
            .ready()
            .await?
            .call(request)
            .await?;

        let response = serde_json::from_value::<Self::Response>(json)?;

        Ok(response)
    }
}

#[async_trait]
pub trait Requestable where Self : Serialize + Sized {
    type Response : DeserializeOwned;

    fn timeout(&self) -> Duration {
        Duration::from_secs(3)
    }

    fn into_request(self) -> anyhow::Result<Request> {
        let timeout = self.timeout();

        Request::with_timeout(self, timeout)
    }
}

impl<T> Callable for T where T : Requestable {
    type Response = T::Response;
}

pub trait Routable {
    fn route(&self) -> Route;
}

#[derive(new, Debug)]
pub struct Forward<Req : Requestable> {
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

    fn into_request(self) -> anyhow::Result<Request> {
        self.req.into_request()
    }
}

impl<Req> Routable for Forward<Req> where Req : Requestable {
    fn route(&self) -> Route { self.route }
}

#[derive(new)]
pub struct RequestableWrapper<T> {
    pub inner: T
}

impl<T> TryFrom<RequestableWrapper<T>> for Request where T : Requestable {
    type Error = anyhow::Error;

    fn try_from(req: RequestableWrapper<T>) -> Result<Self, Self::Error> {
        req.inner.into_request()
    }
}

// TODO[akostylev0]
impl Requestable for Value {
    type Response = Value;
}


pub type RequestId = Uuid;

#[derive(Serialize, Clone)]
pub struct Request {
    #[serde(rename="@extra")]
    pub id: RequestId,

    #[serde(skip_serializing)]
    pub timeout: Duration,

    #[serde(flatten)]
    pub data: Value
}

#[derive(Deserialize, Debug)]
pub struct Response {
    #[serde(rename="@extra")]
    pub id: RequestId,

    #[serde(flatten)]
    pub data: Value
}

impl Request {
    pub fn new<T: Serialize>(data: T) -> anyhow::Result<Self> {
        Self::with_timeout(data, Duration::from_secs(3))
    }

    pub fn with_timeout<T: Serialize>(data: T, timeout: Duration) -> anyhow::Result<Self> {
        Ok(Self {
            id: RequestId::new_v4(),
            timeout,
            data: serde_json::to_value(data)?
        })
    }

    pub fn with_new_id(&self) -> Self {
        Self {
            id: RequestId::new_v4(),
            timeout: self.timeout,
            data: self.data.clone()
        }
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
            data: json!({
                "data": "is flatten"
            })
        };

        assert_eq!(serde_json::to_string(&request).unwrap(), "{\"@extra\":\"7431f198-7514-40ff-876c-3e8ee0a311ba\",\"data\":\"is flatten\"}")
    }
}
