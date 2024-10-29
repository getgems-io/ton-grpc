use crate::block::TonError;
use crate::request::Requestable;
use anyhow::anyhow;
use dashmap::DashMap;
use futures::ready;
use pin_project::{pin_project, pinned_drop};
use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::future::Future;
use std::marker::PhantomData;
use std::pin::Pin;
use std::sync::Arc;
use std::task::{Context, Poll};
use std::time::Duration;
use tokio::sync::oneshot;
use tokio_util::sync::{CancellationToken, DropGuard};
use tower::Service;
use uuid::Uuid;

type RequestStorage = DashMap<RequestId, oneshot::Sender<Response>>;

#[derive(Debug, Clone)]
pub(crate) struct Client {
    client: Arc<tonlibjson_sys::Client>,
    responses: Arc<RequestStorage>,
    drop_guard: Arc<DropGuard>,
}

impl Client {
    pub(crate) fn set_logging(level: i32) {
        tonlibjson_sys::Client::set_verbosity_level(level);
    }

    pub(crate) fn new() -> Self {
        let client = Arc::new(tonlibjson_sys::Client::new());
        let client_recv = client.clone();

        let responses: Arc<RequestStorage> = Default::default();
        let responses_rcv = Arc::clone(&responses);

        let cancel_token = CancellationToken::new();
        let child_token = cancel_token.child_token();

        std::thread::spawn(move || {
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
            drop_guard: Arc::new(cancel_token.drop_guard()),
        }
    }
}

impl Default for Client {
    fn default() -> Self {
        Self::new()
    }
}

impl<R> Service<R> for Client
where
    R: Requestable,
{
    type Response = R::Response;
    type Error = anyhow::Error;
    type Future = ResponseFuture<R::Response>;

    fn poll_ready(&mut self, _: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        Poll::Ready(Ok(()))
    }

    fn call(&mut self, req: R) -> Self::Future {
        let req = Request {
            id: RequestId::new_v4(),
            body: req,
        };

        match serde_json::to_string(&req) {
            Ok(json) => {
                let (tx, rx) = oneshot::channel::<Response>();
                self.responses.insert(req.id, tx);

                match self.client.send(&json) {
                    Ok(_) => ResponseFuture::new(
                        rx,
                        Arc::clone(&self.drop_guard),
                        req.id,
                        Arc::clone(&self.responses),
                    ),
                    Err(e) => ResponseFuture::failed(e),
                }
            }
            Err(e) => ResponseFuture::failed(anyhow!(e)),
        }
    }
}

#[pin_project(project = ResponseStateProj)]
enum ResponseState {
    Failed {
        error: Option<anyhow::Error>,
    },
    Rx {
        #[pin]
        rx: oneshot::Receiver<Response>,
        drop_guard: Arc<DropGuard>,
        request_id: RequestId,
        request_storage: Arc<RequestStorage>,
    },
}

#[pin_project(PinnedDrop)]
pub struct ResponseFuture<R> {
    #[pin]
    state: ResponseState,
    _phantom: PhantomData<R>,
}

#[pinned_drop]
impl<R> PinnedDrop for ResponseFuture<R> {
    fn drop(self: Pin<&mut Self>) {
        if let ResponseState::Rx {
            request_id,
            request_storage,
            ..
        } = &self.state
        {
            request_storage.remove(request_id);
        }
    }
}

impl<R> ResponseFuture<R> {
    fn new(
        rx: oneshot::Receiver<Response>,
        drop_guard: Arc<DropGuard>,
        request_id: RequestId,
        request_storage: Arc<RequestStorage>,
    ) -> Self {
        Self {
            state: ResponseState::Rx {
                rx,
                drop_guard,
                request_id,
                request_storage,
            },
            _phantom: PhantomData,
        }
    }

    fn failed(error: anyhow::Error) -> Self {
        Self {
            state: ResponseState::Failed { error: Some(error) },
            _phantom: PhantomData,
        }
    }
}

impl<R> Future for ResponseFuture<R>
where
    R: DeserializeOwned,
{
    type Output = Result<R, anyhow::Error>;

    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        let mut this = self.project();

        return match this.state.as_mut().project() {
            ResponseStateProj::Failed { error } => {
                Poll::Ready(Err(error.take().expect("polled after error")))
            }
            ResponseStateProj::Rx { rx, .. } => {
                return match ready!(rx.poll(cx)) {
                    Ok(response) => {
                        // TODO[akostylev0] refac!!
                        if response.data["@type"] == "error" {
                            tracing::trace!("Error occurred: {:?}", &response.data);
                            let error = serde_json::from_value::<TonError>(response.data)?;

                            Poll::Ready(Err(error.into()))
                        } else {
                            let data = response.data.clone();
                            let response =
                                serde_json::from_value::<R>(response.data).map_err(|e| {
                                    anyhow!("deserialization error: {:?}, data: {:?}", e, data)
                                })?;

                            Poll::Ready(Ok(response))
                        }
                    }
                    Err(_) => Poll::Ready(Err(anyhow!("oneshot closed"))),
                };
            }
        };
    }
}

type RequestId = Uuid;

#[derive(Serialize)]
struct Request<T: Serialize> {
    #[serde(rename = "@extra")]
    id: RequestId,

    #[serde(flatten)]
    body: T,
}

#[derive(Deserialize, Debug)]
struct Response {
    #[serde(rename = "@extra")]
    id: RequestId,

    #[serde(flatten)]
    data: Value,
}

#[cfg(test)]
mod tests {
    use crate::block::BlocksGetMasterchainInfo;
    use crate::client::{Client, Request};
    use serde_json::json;
    use std::str::FromStr;
    use tower::ServiceExt;
    use tracing_test::traced_test;
    use uuid::Uuid;

    #[tokio::test]
    #[traced_test]
    #[ignore]
    async fn not_initialized_call() {
        let mut client = Client::default();

        let resp = (&mut client)
            .oneshot(BlocksGetMasterchainInfo::default())
            .await;

        assert_eq!(
            "Ton error occurred with code 400, message library is not inited",
            resp.unwrap_err().to_string()
        )
    }

    #[test]
    fn data_is_flatten() {
        let request = Request {
            id: Uuid::from_str("7431f198-7514-40ff-876c-3e8ee0a311ba").unwrap(),
            body: json!({
                "data": "is flatten"
            }),
        };

        assert_eq!(
            serde_json::to_string(&request).unwrap(),
            "{\"@extra\":\"7431f198-7514-40ff-876c-3e8ee0a311ba\",\"data\":\"is flatten\"}"
        )
    }
}
