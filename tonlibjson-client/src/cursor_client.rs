use std::cmp::Ordering;
use std::future::Future;
use std::mem;
use std::ops::Range;
use std::pin::Pin;
use std::task::{Context, Poll, ready};
use std::time::Duration;
use tower::{Service, ServiceExt};
use anyhow::{anyhow, Result};
use async_stream::stream;
use futures::future::BoxFuture;
use futures::{FutureExt, StreamExt, TryFutureExt, TryStreamExt};
use serde_json::Value;
use tokio::sync::watch::Receiver;
use tokio::time::{Interval, interval, MissedTickBehavior};
use tower::limit::ConcurrencyLimit;
use tower::load::{CompleteOnResponse, PeakEwma};
use tower::load::peak_ewma::Cost;
use tracing::{error, info};
use tracing::log::{log, warn};
use tracing_subscriber::fmt::time;
use crate::block::{BlockHeader, BlockId, BlockIdExt};
use crate::client::Client;
use crate::session::{SessionClient, SessionRequest};

enum State {
    Init,
    Future(Pin<Box<dyn Future<Output=(Result<Value>)> + Send>>),
    Ready
}

pub struct CursorClient {
    client: ConcurrencyLimit<SessionClient>,

    first_block: Option<BlockHeader>,

    state: State,

    last_block_rx: Receiver<Option<BlockHeader>>
}

impl CursorClient {
    pub fn new(client: ConcurrencyLimit<SessionClient>) -> Self {
        let (tx, rx) = tokio::sync::watch::channel(None);

        tokio::spawn({
            let mut client = client.clone();
            async move {
                let mut timer = interval(Duration::new(2, 1_000_000_000 / 2));
                timer.set_missed_tick_behavior(MissedTickBehavior::Skip);
                loop {
                    timer.tick().await;

                    let last_block = client
                        .ready()
                        .await
                        .map_err(|e| anyhow!(e))
                        .unwrap()
                        .call(SessionRequest::Synchronize {})
                        .await
                        .map(|val| serde_json::from_value::<BlockHeader>(val).unwrap());

                    if let Ok(last_block) = last_block {
                        info!("new block seqno: {}", last_block.id.seqno);
                        tx.send(Some(last_block)).unwrap();
                    }
                }
            }
        });

        Self {
            client,
            first_block: None,
            state: State::Init,

            last_block_rx: rx
        }
    }
}

impl Service<SessionRequest> for CursorClient {
    type Response = <SessionClient as Service<SessionRequest>>::Response;
    type Error = <SessionClient as Service<SessionRequest>>::Error;
    type Future = <SessionClient as Service<SessionRequest>>::Future;

    fn poll_ready(&mut self, cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        self.state = match &mut self.state {
            State::Init => {
                let mut client = self.client.clone();

                State::Future(async move {
                    let req = futures::stream::iter(vec![SessionRequest::FindFirsBlock {}]);
                    let mut resp = client.call_all(req)
                        .map_err(|e| anyhow!(e));

                    let lhs = resp.next().await.unwrap();

                    (lhs)
                }.boxed())
            },
            State::Future(fut) => {
                let first_block = ready!(fut.poll_unpin(cx));

                match first_block {
                    Ok(f) => {
                        let f = serde_json::from_value::<BlockHeader>(f).unwrap();
                        self.first_block.replace(f);

                        State::Ready
                    },
                    Err(e) => {
                        error!("error occurred during client initialization: {}", e);

                        State::Init
                    },
                }
            },
            State::Ready => {
                if self.last_block_rx.borrow().is_some() {
                    return self.client.poll_ready(cx)
                }

                State::Ready
            }
        };

        cx.waker().wake_by_ref();

        Poll::Pending
    }

    fn call(&mut self, req: SessionRequest) -> Self::Future {
        self.client.call(req).boxed()
    }
}


impl tower::load::Load for CursorClient {
    type Metric = Metrics;

    fn load(&self) -> Self::Metric {
        let last_block = self.last_block_rx.borrow().clone();

        Metrics {
            first_block: self.first_block.clone(),
            last_block,
            ewma: Some(self.client.load())
        }
    }
}

#[derive(Debug)]
pub struct Metrics {
    pub first_block: Option<BlockHeader>,
    pub last_block: Option<BlockHeader>,
    pub ewma: Option<Cost>
}

impl PartialEq<Self> for Metrics {
    fn eq(&self, other: &Self) -> bool {
        self.ewma.eq(&other.ewma)
    }
}

impl PartialOrd<Self> for Metrics {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        self.ewma.partial_cmp(&other.ewma)
    }
}
