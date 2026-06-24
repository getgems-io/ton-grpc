use crate::tl::{BlocksGetMasterchainInfo, Requestable, TonError};
use anyhow::{Context as ErrorContext, anyhow};
use dashmap::DashMap;
use futures::{FutureExt, ready};
use pin_project::{pin_project, pinned_drop};
use serde::de::{DeserializeOwned, Deserializer, Error as DeError};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use std::fmt::Debug;
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

pub struct TonlibjsonClient {
    client: Arc<tonlibjson_sys::Client>,
    responses: Arc<RequestStorage>,
    drop_guard: Arc<DropGuard>,

    state: watch::Receiver<State>,
    ready: Option<Pin<Box<dyn Future<Output = Result<(), watch::error::RecvError>> + Send + Sync>>>,
}

impl Debug for TonlibjsonClient {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("TonlibjsonClient")
            .field("client", &self.client)
            .field("responses", &self.responses)
            .field("drop_guard", &self.drop_guard)
            .field("state", &self.state)
            .finish()
    }
}

impl Clone for TonlibjsonClient {
    fn clone(&self) -> Self {
        Self {
            client: self.client.clone(),
            responses: self.responses.clone(),
            drop_guard: self.drop_guard.clone(),
            state: self.state.clone(),
            ready: None,
        }
    }
}

#[derive(Debug, Copy, Clone)]
enum State {
    Connecting,
    Connected,
    Disconnected,
}

impl TonlibjsonClient {
    pub fn set_logging(level: i32) {
        tonlibjson_sys::Client::set_verbosity_level(level);
    }

    pub fn new(config: TonConfig) -> anyhow::Result<Self> {
        let client = Arc::new(tonlibjson_sys::Client::new());
        let client_recv = client.clone();

        let responses: Arc<RequestStorage> = Default::default();
        let responses_rcv = Arc::clone(&responses);

        let cancel_token = CancellationToken::new();
        let child_token = cancel_token.child_token();

        let ping_id = Uuid::new_v4();
        let ping = Request::with_id(ping_id, BlocksGetMasterchainInfo::default());
        let ping_encoded = serde_json::to_string(&ping).context("failed to encode ping message")?;

        client
            .send(init_config(config).to_string().as_str())
            .context("failed to send init message")?;
        client_recv
            .send(&ping_encoded)
            .context("failed to send ping message")?;

        let (tx, rx) = watch::channel(State::Connecting);
        std::thread::spawn(move || {
            let timeout = Duration::from_secs(1);
            let mut should_continue = true;

            while should_continue && !child_token.is_cancelled() {
                if let Ok(Some(packet)) = client_recv.receive(timeout) {
                    let msg = serde_json::from_str::<Message>(packet);
                    if let Ok(ref msg) = msg {
                        if msg.is_disconnected_error() {
                            tracing::error!(?msg, "disconnected");
                            tx.send_replace(State::Disconnected);
                            should_continue = false;
                        } else if msg.is_connected_notification() {
                            tracing::info!("inner client initialized");
                        }
                    }

                    match msg {
                        Ok(Message::Response(response)) if response.id == ping_id => {
                            tracing::trace!("ping response: {:?}", response);
                            if response.data.is_ok() {
                                tracing::info!("client connected");
                                tx.send_replace(State::Connected);
                            }
                        }
                        Ok(Message::Response(response)) => {
                            if let Some((_, sender)) = responses_rcv.remove(&response.id) {
                                let _ = sender.send(response);
                            }
                        }
                        Ok(Message::Notification(Ok(value))) => {
                            tracing::trace!("Notification: {:?}", value);
                        }
                        Ok(Message::Notification(Err(error))) => {
                            tracing::warn!("Unsolicited error: {:?}", error);
                        }
                        Err(e) => {
                            tracing::warn!("Failed to parse packet: {:?}, packet: {:?}", e, packet);
                        }
                    }
                }
            }

            tracing::trace!("recv thread exited");
        });

        Ok(TonlibjsonClient {
            client,
            responses,
            drop_guard: Arc::new(cancel_token.drop_guard()),
            state: rx,
            ready: None,
        })
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
        loop {
            if let Some(ref mut fut) = self.ready {
                let changed = ready!(fut.poll_unpin(cx));
                self.ready = None;

                if let Err(e) = changed {
                    return Poll::Ready(Err(e.into())); // TODO[akostylev0]: typed error
                }
            }

            let state = *self.state.borrow_and_update();
            match state {
                State::Connected => return Poll::Ready(Ok(())),
                State::Connecting => {
                    let mut rx = self.state.clone();
                    self.ready = Some(Box::pin(async move { rx.changed().await }));
                }
                State::Disconnected => {
                    return Poll::Ready(Err(anyhow::anyhow!("connection is closed")));
                } // TODO[akostylev0]: typed error
            }
        }
    }

    fn call(&mut self, req: R) -> Self::Future {
        let req = Request::new(req);

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

        match this.state.as_mut().project() {
            ResponseStateProj::Failed { error } => {
                Poll::Ready(Err(error.take().expect("polled after error")))
            }
            ResponseStateProj::Rx { rx, .. } => match ready!(rx.poll(cx)) {
                Ok(response) => Poll::Ready(
                    response
                        .data
                        .inspect_err(|error| tracing::trace!("Error occurred: {:?}", error))
                        .map_err(anyhow::Error::from)
                        .and_then(|data| {
                            serde_json::from_value::<R>(data).map_err(anyhow::Error::from)
                        }),
                ),
                Err(_) => Poll::Ready(Err(anyhow!("oneshot closed"))),
            },
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

impl<T> Request<T>
where
    T: Serialize,
{
    pub fn new(body: T) -> Self {
        Self {
            id: Uuid::new_v4(),
            body,
        }
    }

    pub fn with_id(id: RequestId, body: T) -> Self {
        Self { id, body }
    }
}

#[derive(Debug)]
enum Message {
    Response(Response),
    Notification(Result<Value, TonError>),
}

impl Message {
    pub fn is_disconnected_error(&self) -> bool {
        match self {
            Message::Response(Response {
                data: Err(error), ..
            }) => error.is_disconnected(),
            Message::Notification(Err(error)) => error.is_disconnected(),
            _ => false,
        }
    }

    pub fn is_connected_notification(&self) -> bool {
        if let Message::Notification(Ok(value)) = self {
            return value["@type"] == "options.info";
        }

        false
    }
}

impl<'de> Deserialize<'de> for Message {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let mut value = Value::deserialize(deserializer)?;

        let id = value
            .as_object_mut()
            .and_then(|obj| obj.remove("@extra"))
            .map(serde_json::from_value::<RequestId>)
            .transpose()
            .map_err(D::Error::custom)?;

        let data = classify_data(value).map_err(D::Error::custom)?;

        Ok(match id {
            Some(id) => Message::Response(Response { id, data }),
            None => Message::Notification(data),
        })
    }
}

#[derive(Debug)]
struct Response {
    id: RequestId,
    data: Result<Value, TonError>,
}

fn classify_data(value: Value) -> Result<Result<Value, TonError>, serde_json::Error> {
    let is_error = value
        .get("@type")
        .and_then(Value::as_str)
        .is_some_and(|t| t == "error");

    if is_error {
        let err = serde_json::from_value::<TonError>(value)?;
        Ok(Err(err))
    } else {
        Ok(Ok(value))
    }
}

fn init_config(config: TonConfig) -> Value {
    json!({
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
    })
}

#[cfg(test)]
mod tests {
    use crate::client::{Message, Request};
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

    #[test]
    fn message_with_extra_ok_is_response() {
        let payload = json!({
            "@extra": "7431f198-7514-40ff-876c-3e8ee0a311ba",
            "@type": "ok",
            "value": 42
        });

        let message: Message = serde_json::from_value(payload).unwrap();

        let Message::Response(response) = message else {
            panic!("expected Response variant, got {:?}", message);
        };
        let data = response.data.expect("expected Ok data");
        assert_eq!(data["@type"], "ok");
        assert_eq!(data["value"], 42);
    }

    #[test]
    fn message_with_extra_error_is_response_err() {
        let payload = json!({
            "@extra": "7431f198-7514-40ff-876c-3e8ee0a311ba",
            "@type": "error",
            "code": 400,
            "message": "library is not inited"
        });

        let message: Message = serde_json::from_value(payload).unwrap();

        let Message::Response(response) = message else {
            panic!("expected Response variant, got {:?}", message);
        };
        let err = response.data.expect_err("expected Err data");
        assert_eq!(
            err.to_string(),
            "Ton error occurred with code 400, message library is not inited"
        );
    }

    #[test]
    fn message_without_extra_ok_is_notification() {
        let payload = json!({
            "@type": "updateSyncState",
            "sync_state": { "@type": "syncStateDone" }
        });

        let message: Message = serde_json::from_value(payload).unwrap();

        let Message::Notification(data) = message else {
            panic!("expected Notification variant, got {:?}", message);
        };
        let value = data.expect("expected Ok data");
        assert_eq!(value["@type"], "updateSyncState");
    }

    #[test]
    fn message_without_extra_error_is_notification_err() {
        let payload = json!({
            "@type": "error",
            "code": 500,
            "message": "internal"
        });

        let message: Message = serde_json::from_value(payload).unwrap();

        let Message::Notification(data) = message else {
            panic!("expected Notification variant, got {:?}", message);
        };
        let err = data.expect_err("expected Err data");
        assert_eq!(
            err.to_string(),
            "Ton error occurred with code 500, message internal"
        );
    }
}
