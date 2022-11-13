use std::future::Future;
use std::pin::Pin;
use std::sync::{Arc, mpsc, Mutex};
use std::sync::mpsc::TryRecvError;
use std::task::{Context, Poll};
use std::time::Duration;
use anyhow::anyhow;
use serde_json::Value;
use dashmap::DashMap;
use tower::Service;
use tonlibjson_rs::Client;
use crate::TonError;
use crate::request::{Request, RequestId, Response};

#[derive(Clone, Debug)]
pub struct AsyncClient {
    client: Arc<Client>,
    responses: Arc<DashMap<RequestId, tokio::sync::oneshot::Sender<Response>>>,
    _stop_signal: Arc<Mutex<Stop>>
}

impl AsyncClient {
    pub fn new() -> Self {
        let client = Arc::new(Client::new());
        let client_recv = client.clone();

        let responses: Arc<DashMap<RequestId, tokio::sync::oneshot::Sender<Response>>> =
            Arc::new(DashMap::new());
        let responses_rcv = Arc::clone(&responses);
        let (stop_signal, stop_receiver) = mpsc::channel();

        let _ = tokio::task::spawn_blocking(move || {
            let timeout = Duration::from_secs(20);

            loop {
                match stop_receiver.try_recv() {
                    Ok(_) | Err(TryRecvError::Disconnected) => {
                        tracing::warn!("Stop thread");
                        return
                    },
                    Err(TryRecvError::Empty) => {
                        if let Ok(packet) = client_recv.receive(timeout) {
                            if let Ok(response) = serde_json::from_str::<Response>(packet) {
                                if let Some((_, sender)) = responses_rcv.remove(&response.id) {
                                    let _ = sender.send(response);
                                }
                            } else if !packet.contains("syncState") {
                                tracing::warn!("Unexpected response {:?}", packet.to_string())
                            }
                        }
                    }
                }
            }
        });

        AsyncClient {
            client,
            responses,
            _stop_signal: Arc::new(Mutex::new(Stop::new(stop_signal)))
        }
    }

    async fn execute(&self, request: &Request) -> anyhow::Result<Value> {
        let (tx, rx) = tokio::sync::oneshot::channel::<Response>();
        self.responses.insert(request.id, tx);

        let _ = self.client.send(&serde_json::to_string(request)?);

        let result = tokio::time::timeout(request.timeout, rx).await;
        self.responses.remove(&request.id);

        let response = result??;

        // TODO[akostylev0] refac
        if response.data["@type"] == "error" {
            tracing::warn!("Error occurred: {:?}", &response.data);
            let error = serde_json::from_value::<TonError>(response.data)?;

            return Err(anyhow!(error))
        }

        Ok(response.data)
    }
}

impl Default for AsyncClient {
    fn default() -> Self {
        Self::new()
    }
}

impl Service<Request> for AsyncClient {
    type Response = Value;
    type Error = anyhow::Error;
    type Future = Pin<Box<dyn Future<Output = Result<Self::Response, Self::Error>> + Send>>;

    fn poll_ready(&mut self, _: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        Poll::Ready(Ok(()))
    }

    fn call(&mut self, req: Request) -> Self::Future {
        let this = self.clone();

        Box::pin(async move {
            this.execute(&req).await
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
