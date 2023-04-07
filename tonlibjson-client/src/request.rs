use std::time::Duration;
use async_trait::async_trait;
use derive_new::new;
use uuid::Uuid;
use serde::{Serialize, Deserialize, Serializer};
use serde::de::DeserializeOwned;
use serde_json::Value;
use tower::{Service, ServiceExt};
use crate::balance::{BalanceRequest, Route};
use crate::block::{BlocksGetBlockHeader, BlocksGetShards, BlocksGetTransactions, BlocksLookupBlock, GetAccountState, GetMasterchainInfo, GetShardAccountCell, RawGetAccountState, RawGetAccountStateByTransaction, RawGetTransactionsV2, RawSendMessage, SmcLoad, SmcRunGetMethod, Sync};
use crate::session::SessionRequest;

#[async_trait]
pub trait Callable : Sized {
    type Response : DeserializeOwned;

    async fn call<Req, S : Service<Req, Response = Value, Error = anyhow::Error>>(self, client: &mut S) -> Result<Self::Response, S::Error>
        where Req: Send,
              S : Send,
              S::Future : Send,
              CallableWrapper<Self> : Into<Req>
    {
        let request = CallableWrapper::new(self).into();

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

    fn into_request_body(self) -> RequestBody;
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

    fn into_request_body(self) -> RequestBody {
        self.req.into_request_body()
    }
}

impl<Req> Routable for Forward<Req> where Req : Requestable {
    fn route(&self) -> Route { self.route }
}

#[derive(new)]
pub struct CallableWrapper<T> {
    pub inner: T
}

impl<T> From<CallableWrapper<T>> for Request where T : Requestable {
    fn from(req: CallableWrapper<T>) -> Self {
        let timeout = req.inner.timeout();
        let body = req.inner.into_request_body();

        Request::new(body, timeout)
    }
}

impl<T> From<CallableWrapper<T>> for SessionRequest where T : Requestable {
    fn from(req: CallableWrapper<T>) -> Self {
        SessionRequest::new_atomic(req.into())
    }
}

impl<T> From<CallableWrapper<T>> for BalanceRequest where T : Routable, CallableWrapper<T> : Into<SessionRequest> {
    fn from(req: CallableWrapper<T>) -> Self {
        let route = req.inner.route();

        BalanceRequest::new(route, req.into())
    }
}

impl Requestable for Value {
    type Response = Value;

    fn into_request_body(self) -> RequestBody {
        RequestBody::Value(self)
    }
}

pub type RequestId = Uuid;

#[derive(Clone, Serialize)]
#[serde(untagged)]
pub enum RequestBody {
    Sync(Sync),

    GetMasterchainInfo(GetMasterchainInfo),

    GetAccountState(GetAccountState),
    GetShardAccountCell(GetShardAccountCell),

    BlocksGetShards(BlocksGetShards),
    BlocksGetBlockHeader(BlocksGetBlockHeader),
    BlocksLookupBlock(BlocksLookupBlock),
    BlocksGetTransactions(BlocksGetTransactions),

    RawSendMessage(RawSendMessage),
    RawGetAccountState(RawGetAccountState),
    RawGetAccountStateByTransaction(RawGetAccountStateByTransaction),
    RawGetTransactionsV2(RawGetTransactionsV2),

    SmcLoad(SmcLoad),
    SmcRunGetMethod(SmcRunGetMethod),

    Value(Value)
}


#[derive(Serialize, Clone)]
pub struct Request {
    #[serde(rename="@extra")]
    pub id: RequestId,

    #[serde(skip_serializing)]
    pub timeout: Duration,

    #[serde(flatten)]
    pub body: RequestBody
}

#[derive(Deserialize, Debug)]
pub struct Response {
    #[serde(rename="@extra")]
    pub id: RequestId,

    #[serde(flatten)]
    pub data: Value
}

impl Request {
    pub fn new(body: RequestBody, timeout: Duration) -> Self {
        Self {
            id: RequestId::new_v4(),
            timeout,
            body
        }
    }

    pub fn with_new_id(&self) -> Self {
        Self {
            id: RequestId::new_v4(),
            timeout: self.timeout,
            body: self.body.clone()
        }
    }
}

#[cfg(test)]
mod tests {
    use std::str::FromStr;
    use std::time::Duration;
    use serde_json::json;
    use uuid::Uuid;
    use crate::request::{Request, RequestBody};

    #[test]
    fn data_is_flatten() {
        let request = Request {
            id: Uuid::from_str("7431f198-7514-40ff-876c-3e8ee0a311ba").unwrap(),
            timeout: Duration::from_secs(3),
            body: RequestBody::Value(json!({
                "data": "is flatten"
            }))
        };

        assert_eq!(serde_json::to_string(&request).unwrap(), "{\"@extra\":\"7431f198-7514-40ff-876c-3e8ee0a311ba\",\"data\":\"is flatten\"}")
    }
}
