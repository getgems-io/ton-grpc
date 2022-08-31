use std::future::Future;
use std::pin::Pin;
use std::thread;
use std::sync::{Arc, mpsc, Mutex};
use std::sync::mpsc::TryRecvError;
use std::task::{Context, Poll};
use std::time::Duration;
use serde_json::{json, Value};
use anyhow::anyhow;
use dashmap::DashMap;
use serde::de::DeserializeOwned;
use tower::Service;
use uuid::Uuid;
use tonlibjson_rs::Client;
use crate::{ServiceError, TonError};

#[allow(dead_code)]
#[derive(Clone, Debug)]
pub struct AsyncClient {
    client: Arc<Client>,
    responses: Arc<DashMap<String, tokio::sync::oneshot::Sender<Value>>>,
    stop_signal: Arc<Mutex<Stop>>
}

impl AsyncClient {
    pub fn new() -> Self {
        let client = Arc::new(Client::new());
        let client_recv = client.clone();

        let responses: Arc<DashMap<String, tokio::sync::oneshot::Sender<Value>>> =
            Arc::new(DashMap::new());
        let responses_rcv = Arc::clone(&responses);
        let (stop_signal, stop_receiver) = mpsc::channel();

        let _ = Arc::new(thread::spawn(move || {
            let timeout = Duration::from_secs(20);
            loop {
                match stop_receiver.try_recv() {
                    Ok(_) | Err(TryRecvError::Disconnected) => {
                        tracing::warn!("Stop thread");
                        break
                    },
                    Err(TryRecvError::Empty) => {
                        if let Ok(packet) = client_recv.receive(timeout) {
                            if let Ok(json) = serde_json::from_str::<Value>(packet) {
                                if let Some(Value::String(ref id)) = json.get("@extra") {
                                    if let Some((_, s)) = responses_rcv.remove(id) {
                                        let _ = s.send(json);
                                    }
                                } else if let Some(Value::String(ref typ)) = json.get("@type") {
                                    match typ.as_str() {
                                        "updateSyncState" => (),
                                        _ => tracing::warn!("Unexpected response {:?} with type {}", json.to_string(), typ)
                                    }
                                } else {
                                    tracing::warn!("Unexpected response {:?}", json.to_string())
                                }
                            }
                        }
                    }
                }
            }
        }));

        AsyncClient {
            client,
            responses,
            stop_signal: Arc::new(Mutex::new(Stop::new(stop_signal)))
        }
    }

    pub async fn execute(&self, request: Value) -> anyhow::Result<Value> {
        self.execute_typed_with_timeout(&request, Duration::from_secs(20))
            .await
    }

    async fn execute_typed_with_timeout<T: DeserializeOwned>(
        &self,
        request: &Value,
        timeout: Duration,
    ) -> anyhow::Result<T> {
        let mut request = request.clone();

        let id = Uuid::new_v4().to_string();
        request["@extra"] = json!(id);
        let (tx, rx) = tokio::sync::oneshot::channel::<Value>();
        self.responses.insert(id.clone(), tx);

        let x = request.to_string();
        let _ = self.client.send(&x);

        let timeout = tokio::time::timeout(timeout, rx).await?;

        match timeout {
            Ok(mut value) => {
                let obj = value.as_object_mut().ok_or_else(||anyhow!("Not an object"))?;
                let _ = obj.remove("@extra");

                if value["@type"] == "error" {
                    tracing::warn!("Error occurred: {:?}", &value);
                    return match serde_json::from_value::<TonError>(value) {
                        Ok(e) => Err(anyhow::Error::from(e)),
                        Err(e) => Err(anyhow::Error::from(e)),
                    };
                }

                serde_json::from_value::<T>(value).map_err(anyhow::Error::from)
            }
            Err(e) => {
                tracing::warn!("timeout reached");
                self.responses.remove(&id);

                Err(anyhow::Error::from(e))
            }
        }
    }

    pub async fn synchronize(&self) -> anyhow::Result<Value> {
        let query = json!({
            "@type": "sync"
        });

        self.execute_typed_with_timeout::<Value>(&query, Duration::from_secs(60 * 5))
            .await
    }
}

impl Default for AsyncClient {
    fn default() -> Self {
        Self::new()
    }
}

impl Service<Value> for AsyncClient {
    type Response = Value;
    type Error = ServiceError;
    type Future = Pin<Box<dyn Future<Output = Result<Self::Response, Self::Error>> + Send>>;

    fn poll_ready(&mut self, _: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        Poll::Ready(Ok(()))
    }

    fn call(&mut self, req: Value) -> Self::Future {
        let this = self.clone();

        Box::pin(async move {
            this.execute(req).await.map_err(ServiceError::from)
        })
    }
}

#[derive(Debug)]
struct Stop {
    sender: mpsc::Sender<()>
}

impl Stop {
    fn new(sender: mpsc::Sender<()>) -> Self {
        Self {
            sender
        }
    }
}

impl Drop for Stop {
    fn drop(&mut self) {
        let _ = self.sender.send(());
    }
}
