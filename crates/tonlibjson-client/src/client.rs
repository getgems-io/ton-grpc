use crate::tl::{Requestable, TonError};
use anyhow::anyhow;
use dashmap::DashMap;
use futures::ready;
use pin_project::{pin_project, pinned_drop};
use serde::de::{DeserializeOwned, Deserializer, Error as DeError};
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
pub struct TonlibjsonClient {
    client: Arc<tonlibjson_sys::Client>,
    responses: Arc<RequestStorage>,
    drop_guard: Arc<DropGuard>,
}

impl TonlibjsonClient {
    pub fn set_logging(level: i32) {
        tonlibjson_sys::Client::set_verbosity_level(level);
    }

    pub fn new() -> Self {
        let client = Arc::new(tonlibjson_sys::Client::new());
        let client_recv = client.clone();

        let responses: Arc<RequestStorage> = Default::default();
        let responses_rcv = Arc::clone(&responses);

        let cancel_token = CancellationToken::new();
        let child_token = cancel_token.child_token();

        std::thread::spawn(move || {
            let timeout = Duration::from_secs(1);
            while !child_token.is_cancelled() {
                if let Ok(Some(packet)) = client_recv.receive(timeout) {
                    match serde_json::from_str::<Message>(packet) {
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

            tracing::trace!("Client dropped");
        });

        TonlibjsonClient {
            client,
            responses,
            drop_guard: Arc::new(cancel_token.drop_guard()),
        }
    }
}

impl<R> Service<R> for TonlibjsonClient
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

#[derive(Debug)]
enum Message {
    Response(Response),
    Notification(Result<Value, TonError>),
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
