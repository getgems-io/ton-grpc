use std::cmp::Ordering;
use std::task::{Context, Poll};
use std::time::Duration;
use tower::{Service, ServiceExt};
use anyhow::{anyhow, Result};
use futures::{FutureExt};
use tokio::sync::watch::Receiver;
use tokio::time::{interval, MissedTickBehavior};
use tower::limit::ConcurrencyLimit;
use tower::load::peak_ewma::Cost;
use tracing::{info};
use crate::block::{BlockHeader, BlocksLookupBlock};
use crate::request::Request;
use crate::session::{SessionClient, SessionRequest};

pub struct CursorClient {
    client: ConcurrencyLimit<SessionClient>,

    first_block_rx: Receiver<Option<BlockHeader>>,
    last_block_rx: Receiver<Option<BlockHeader>>
}

impl CursorClient {
    pub fn new(client: ConcurrencyLimit<SessionClient>) -> Self {
        let (stx, srx) = tokio::sync::watch::channel(None);
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
                        stx.send(Some(last_block)).unwrap();
                    }
                }
            }
        });

        let (ftx, frx) = tokio::sync::watch::channel(None);
        tokio::spawn({
            let mut client = client.clone();
            let mut first_block: Option<BlockHeader> = None;

            async move {
                let mut timer = interval(Duration::from_secs(30));
                timer.set_missed_tick_behavior(MissedTickBehavior::Skip);
                loop {
                    timer.tick().await;

                    if let Some(fb) = first_block.clone() {
                        let request = BlocksLookupBlock::new(fb.into(), 0, 0);
                        let fb = client
                            .ready()
                            .await
                            .map_err(|e| anyhow!(e))
                            .unwrap()
                            .call(SessionRequest::Atomic(Request::new(request).unwrap()))
                            .await;

                        if fb.is_err() {
                            first_block = None;
                        } else {
                            info!("first block still available")
                        }
                    }

                    if first_block.is_none() {
                        let fb = client
                            .ready()
                            .await
                            .map_err(|e| anyhow!(e))
                            .unwrap()
                            .call(SessionRequest::FindFirstBlock {})
                            .await
                            .map(|val| serde_json::from_value::<BlockHeader>(val).unwrap());

                        if let Ok(fb) = fb {
                            info!("new first block seqno: {}", fb.id.seqno);

                            first_block = Some(fb.clone());

                            ftx.send(Some(fb)).unwrap();
                        }
                    }
                }
            }
        });

        Self {
            client,

            first_block_rx: frx,
            last_block_rx: srx
        }
    }
}

impl Service<SessionRequest> for CursorClient {
    type Response = <SessionClient as Service<SessionRequest>>::Response;
    type Error = <SessionClient as Service<SessionRequest>>::Error;
    type Future = <SessionClient as Service<SessionRequest>>::Future;

    fn poll_ready(&mut self, cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        if self.last_block_rx.borrow().is_some()
            && self.first_block_rx.borrow().is_some() {
            return self.client.poll_ready(cx)
        }

        cx.waker().wake_by_ref();

        Poll::Pending
    }

    fn call(&mut self, req: SessionRequest) -> Self::Future {
        self.client.call(req).boxed()
    }
}


impl tower::load::Load for CursorClient {
    type Metric = Option<Metrics>;

    fn load(&self) -> Self::Metric {
        let Some(first_block) = self.first_block_rx.borrow().clone() else {
            return None;
        };
        let Some(last_block) = self.last_block_rx.borrow().clone() else {
            return None;
        };

        Some(Metrics {
            first_block,
            last_block,
            ewma: self.client.load()
        })
    }
}

#[derive(Debug)]
pub struct Metrics {
    pub first_block: BlockHeader,
    pub last_block: BlockHeader,
    pub ewma: Cost
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
