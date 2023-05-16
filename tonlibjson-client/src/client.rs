use std::future::Future;
use std::pin::Pin;
use std::sync::{Arc, mpsc, Mutex};
use std::sync::mpsc::TryRecvError;
use std::task::{Context, Poll};
use std::time::Duration;
use anyhow::anyhow;
use serde_json::Value;
use dashmap::DashMap;
use tower::{Service};
use tracing::trace;
use crate::block::TonError;
use crate::request::{Request, RequestId, Response};

#[derive(Debug)]
pub struct Client {
    client: Arc<tonlibjson_sys::Client>,
    responses: Arc<DashMap<RequestId, tokio::sync::oneshot::Sender<Response>>>,
    _stop_signal: Arc<Mutex<Stop>>
}

impl Client {
    pub fn set_logging(level: i32) {
        tonlibjson_sys::Client::set_verbosity_level(level);
    }

    pub fn new() -> Self {
        let client = Arc::new(tonlibjson_sys::Client::new());
        let client_recv = client.clone();

        let responses: Arc<DashMap<RequestId, tokio::sync::oneshot::Sender<Response>>> =
            Arc::new(DashMap::new());
        let responses_rcv = Arc::clone(&responses);
        let (stop_signal, stop_receiver) = mpsc::channel();

        tokio::task::spawn_blocking(move || {
            let timeout = Duration::from_secs(20);

            loop {
                match stop_receiver.try_recv() {
                    Ok(_) | Err(TryRecvError::Disconnected) => {
                        return
                    },
                    Err(TryRecvError::Empty) => {
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
                }
            }
        });

        Client {
            client,
            responses,
            _stop_signal: Arc::new(Mutex::new(Stop::new(stop_signal)))
        }
    }
}

impl Default for Client {
    fn default() -> Self {
        Self::new()
    }
}

impl Service<Request> for Client {
    type Response = Value;
    type Error = anyhow::Error;
    type Future = Pin<Box<dyn Future<Output = Result<Self::Response, Self::Error>> + Send + Sync>>;

    fn poll_ready(&mut self, _: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        Poll::Ready(Ok(()))
    }

    fn call(&mut self, req: Request) -> Self::Future {
        let to_string = serde_json::to_string(&req);
        let Ok(query) = to_string else {
            return Box::pin(futures::future::ready(Err(anyhow!(to_string.unwrap_err()))));
        };

        let requests = Arc::clone(&self.responses);
        let (tx, rx) = tokio::sync::oneshot::channel::<Response>();
        requests.insert(req.id, tx);

        let sent = self.client.send(&query);

        Box::pin(async move {
            sent?;

            let result = tokio::time::timeout(req.timeout, rx).await;
            requests.remove(&req.id);

            let response = result??;

            // TODO[akostylev0] refac
            if response.data["@type"] == "error" {
                trace!("Error occurred: {:?}", &response.data);
                let error = serde_json::from_value::<TonError>(response.data)?;

                return Err(anyhow!(error))
            }

            Ok(response.data)
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
