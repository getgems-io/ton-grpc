use crate::tl::{BlocksGetMasterchainInfo, Requestable, TonError};
use anyhow::anyhow;
use dashmap::DashMap;
use futures::ready;
use pin_project::{pin_project, pinned_drop};
use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::future::Future;
use std::marker::PhantomData;
use std::pin::Pin;
use std::sync::Arc;
use std::task::{Context, Poll};
use std::time::Duration;
use tokio::sync::{oneshot, watch};
use tokio_util::sync::{CancellationToken, DropGuard};
use ton_config::TonConfig;
use tower::Service;
use uuid::Uuid;

type RequestStorage = DashMap<RequestId, oneshot::Sender<Response>>;

#[derive(Debug)]
enum ConnectionState {
    Connecting,
    Connected,
    Disconnected,
}

#[derive(Clone)]
pub struct TonlibjsonClient {
    state: watch::Receiver<ConnectionState>,
    client: Arc<tonlibjson_sys::Client>,
    responses: Arc<RequestStorage>,
    drop_guard: Arc<DropGuard>,
}

impl TonlibjsonClient {
    pub fn set_logging(level: i32) {
        tonlibjson_sys::Client::set_verbosity_level(level);
    }

    pub fn new(config: TonConfig) -> anyhow::Result<Self> {
        let client = Arc::new(tonlibjson_sys::Client::new());
        let ping_query_id = RequestId::new_v4();
        let ping_query = Request {
            id: ping_query_id,
            body: BlocksGetMasterchainInfo::default(),
        };
        client.send(Self::config(config)?.as_ref())?;
        client.send(&serde_json::to_string(&ping_query)?)?;

        let client_recv = client.clone();

        let responses: Arc<RequestStorage> = Default::default();
        let responses_rcv = Arc::clone(&responses);

        let cancel_token = CancellationToken::new();
        let child_token = cancel_token.child_token();

        let (state_tx, state_rx) = watch::channel(ConnectionState::Connecting);
        tokio::task::spawn(async move {
            while !child_token.is_cancelled() {
                while let Ok(packet) = client_recv.receive(Duration::from_secs(0)) {
                    if let Ok(response) = serde_json::from_str::<Response>(packet) {
                        if response.data["@type"] == "error" {
                            let error =
                                serde_json::from_value::<TonError>(response.data.clone()).unwrap();
                            if error.code() == 500 && error.message() == "LITE_SERVER_NETWORK" {
                                let _ = state_tx.send(ConnectionState::Disconnected);
                            }
                        } else if response.id == ping_query_id {
                            let _ = state_tx.send(ConnectionState::Connected);
                        }

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

                tokio::time::sleep(Duration::from_millis(10)).await;
            }

            tracing::trace!("Client dropped");
        });

        Ok(TonlibjsonClient {
            state: state_rx,
            client,
            responses,
            drop_guard: Arc::new(cancel_token.drop_guard()),
        })
    }

    fn config(config: TonConfig) -> anyhow::Result<String> {
        let full_config = json!({
            "@type": "init",
            "options": {
                "@type": "options",
                "config": {
                    "@type": "config",
                    "config": config.to_string(),
                    "use_callbacks_for_network": false,
                    "blockchain_name": "",
                    "ignore_cache": true
                },
                "keystore_type": {
                    "@type": "keyStoreTypeInMemory"
                }
            }
        });

        serde_json::to_string(&full_config).map_err(Into::into)
    }
}

impl<R> Service<R> for TonlibjsonClient
where
    R: Requestable,
{
    type Response = R::Response;
    type Error = anyhow::Error;
    type Future = ResponseFuture<R::Response>;

    fn poll_ready(&mut self, cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        match *self.state.borrow_and_update() {
            ConnectionState::Connected => Poll::Ready(Ok(())),
            ConnectionState::Disconnected => Poll::Ready(Err(anyhow!(
                "client is disconnected: lite server network failure"
            ))),
            ConnectionState::Connecting => {
                cx.waker().wake_by_ref();
                Poll::Pending
            }
        }
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
            Err(e) => ResponseFuture::failed(e.into()),
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

        match this.state.as_mut().project() {
            ResponseStateProj::Failed { error } => {
                Poll::Ready(Err(error.take().expect("polled after error")))
            }
            ResponseStateProj::Rx { rx, .. } => {
                match ready!(rx.poll(cx)) {
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
                }
            }
        }
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
    use crate::client::Request;
    use serde_json::json;
    use std::str::FromStr;
    use uuid::Uuid;

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
