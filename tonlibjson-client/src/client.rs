use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;
use std::task::{Context, Poll};
use std::time::Duration;
use tower::{Service};
use anyhow::{anyhow, bail};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use dashmap::DashMap;
use tokio_util::sync::{CancellationToken, DropGuard};
use uuid::Uuid;
use crate::block::TonError;
use crate::request::Requestable;

#[derive(Debug)]
pub(crate) struct Client {
    client: Arc<tonlibjson_sys::Client>,
    responses: Arc<DashMap<RequestId, tokio::sync::oneshot::Sender<Response>>>,
    _drop_guard: DropGuard
}

impl Client {
    pub(crate) fn set_logging(level: i32) {
        tonlibjson_sys::Client::set_verbosity_level(level);
    }

    pub(crate) fn new() -> Self {
        let client = Arc::new(tonlibjson_sys::Client::new());
        let client_recv = client.clone();

        let responses: Arc<DashMap<RequestId, tokio::sync::oneshot::Sender<Response>>> =
            Arc::new(DashMap::new());
        let responses_rcv = Arc::clone(&responses);

        let cancel_token = CancellationToken::new();
        let child_token = cancel_token.child_token();

        tokio::task::spawn_blocking(move || {
            let timeout = Duration::from_secs(1);
            while !child_token.is_cancelled() {
                if let Ok(packet) = client_recv.receive(timeout) {
                    if let Ok(response) = serde_json::from_str::<Response>(packet) {
                        if let Some((_, sender)) = responses_rcv.remove(&response.id) {
                            let _ = sender.send(response);
                        }
                    } else if packet.contains("syncState") {
                        tracing::trace!("Sync state: {}", packet.to_string());
                        if packet.contains("syncStateDone") {
                            tracing::trace!("syncState: {:#?}", packet);
                        }
                    } else {
                        tracing::warn!("Unexpected response {:?}", packet.to_string())
                    }
                }
            }

            tracing::trace!("Client dropped");
        });

        Client {
            client,
            responses,
            _drop_guard: cancel_token.drop_guard()
        }
    }
}

impl Default for Client {
    fn default() -> Self {
        Self::new()
    }
}

impl<R : Requestable> Service<R> for Client {
    type Response = R::Response;
    type Error = anyhow::Error;
    type Future = Pin<Box<dyn Future<Output = Result<Self::Response, Self::Error>> + Send + Sync>>;

    fn poll_ready(&mut self, _: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        Poll::Ready(Ok(()))
    }

    fn call(&mut self, req: R) -> Self::Future {
        let req = Request {
            id: RequestId::new_v4(),
            timeout: req.timeout(),
            body: req,
        };

        let to_string = serde_json::to_string(&req);
        let Ok(query) = to_string else {
            return Box::pin(futures::future::ready(Err(anyhow!(to_string.unwrap_err()))));
        };

        let requests = Arc::clone(&self.responses);
        let (tx, rx) = tokio::sync::oneshot::channel::<Response>();
        requests.insert(req.id, tx);

        let sent = self.client.send(&query);
        if let Err(e) = sent {
            return Box::pin(futures::future::ready(Err(e)));
        }

        Box::pin(async move {
            let result = tokio::time::timeout(req.timeout, rx).await;
            requests.remove(&req.id);

            let response = result??;

            // TODO[akostylev0] refac!!
            if response.data["@type"] == "error" {
                tracing::trace!("Error occurred: {:?}", &response.data);
                let error = serde_json::from_value::<TonError>(response.data)?;

                bail!(error)
            } else {
                let data = response.data.clone();
                let response = serde_json::from_value::<R::Response>(response.data)
                    .map_err(|e| anyhow!("deserialization error: {:?}, data: {:?}", e, data))?;

                Ok(response)
            }
        })
    }
}

type RequestId = Uuid;

#[derive(Serialize)]
struct Request<T : Serialize> {
    #[serde(rename="@extra")]
    id: RequestId,

    #[serde(skip_serializing)]
    timeout: Duration,

    #[serde(flatten)]
    body: T
}

#[derive(Deserialize, Debug)]
struct Response {
    #[serde(rename="@extra")]
    id: RequestId,

    #[serde(flatten)]
    data: Value
}

#[cfg(test)]
mod tests {
    use std::str::FromStr;
    use std::time::Duration;
    use serde_json::json;
    use tower::ServiceExt;
    use tracing_test::traced_test;
    use uuid::Uuid;
    use crate::block::BlocksGetMasterchainInfo;
    use crate::client::{Client, Request};

    #[tokio::test]
    #[traced_test]
    #[ignore]
    async fn not_initialized_call() {
        let mut client = Client::default();

        let resp = (&mut client).oneshot(BlocksGetMasterchainInfo::default()).await;

        assert_eq!("Ton error occurred with code 400, message library is not inited", resp.unwrap_err().to_string())
    }

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
