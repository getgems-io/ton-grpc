use std::cmp::Ordering;
use std::task::{Context, Poll};
use std::time::Duration;
use tower::{Service, ServiceExt};
use anyhow::Result;
use futures::{FutureExt, TryFutureExt};
use tokio::sync::watch::Receiver;
use tokio::time::{interval, MissedTickBehavior};
use tower::limit::ConcurrencyLimit;
use tower::load::peak_ewma::Cost;
use tracing::{error, trace};
use crate::block::{BlockHeader, BlocksLookupBlock, MasterchainInfo};
use crate::request::Request;
use crate::session::{SessionClient, SessionRequest};

pub struct CursorClient {
    client: ConcurrencyLimit<SessionClient>,

    first_block_rx: Receiver<Option<BlockHeader>>,
    last_block_rx: Receiver<Option<BlockHeader>>,

    masterchain_info_rx: Receiver<Option<MasterchainInfo>>
}

impl CursorClient {
    pub fn new(client: ConcurrencyLimit<SessionClient>) -> Self {
        let (ctx, crx) = tokio::sync::watch::channel(None);
        let (mtx, mrx) = tokio::sync::watch::channel(None);
        tokio::spawn({
            let mut client = client.clone();
            async move {
                let mut timer = interval(Duration::new(2, 1_000_000_000 / 2));
                timer.set_missed_tick_behavior(MissedTickBehavior::Skip);

                let mut current: Option<MasterchainInfo> = None;
                loop {
                    timer.tick().await;

                    let masterchain_info: Result<MasterchainInfo> = client
                        .ready()
                        .and_then(|c| c.call(SessionRequest::GetMasterchainInfo {}))
                        .map_ok(|val| serde_json::from_value::<MasterchainInfo>(val).unwrap())
                        .await;

                    match masterchain_info {
                        Ok(mut masterchain_info) => {
                            if let Some(cur) = current.clone() {
                                if cur == masterchain_info {
                                    trace!(cursor = cur.last.seqno, "block actual");

                                    continue;
                                } else {
                                    trace!(cursor = cur.last.seqno, actual = masterchain_info.last.seqno, "block discovered")
                                }
                            }

                            let last_block: Result<BlockHeader> = client
                                .ready()
                                .and_then(|c| c.call(SessionRequest::Synchronize {}))
                                .map_ok(|val| serde_json::from_value::<BlockHeader>(val).unwrap())
                                .await;

                            match last_block {
                                Ok(last_block) => {
                                    masterchain_info.last = last_block.id.clone();
                                    trace!(seqno = last_block.id.seqno, "block reached");

                                    current.replace(masterchain_info.clone());

                                    mtx.send(Some(masterchain_info)).unwrap();
                                    ctx.send(Some(last_block)).unwrap();
                                },
                                Err(e) => error!("{}", e)
                            }
                        },
                        Err(e) => error!("{}", e)
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
                        let request = BlocksLookupBlock::seqno(fb.into());
                        let fb = client
                            .ready()
                            .and_then(|c| c.call(SessionRequest::Atomic(Request::new(request).unwrap())))
                            .await;

                        if let Err(e) = fb {
                            error!("{}", e);
                            first_block = None;
                        } else {
                            trace!("first block still available")
                        }
                    }

                    if first_block.is_none() {
                        let fb = client
                            .ready()
                            .and_then(|c| c.call(SessionRequest::FindFirstBlock {}))
                            .map_ok(|val| serde_json::from_value::<BlockHeader>(val).unwrap())
                            .await;

                        match fb {
                            Ok(fb) => {
                                trace!("new first block seqno: {}", fb.id.seqno);

                                first_block = Some(fb.clone());

                                ftx.send(Some(fb)).unwrap();
                            },
                            Err(e) => error!("{}", e)
                        }
                    }
                }
            }
        });

        Self {
            client,

            first_block_rx: frx,
            last_block_rx: crx,
            masterchain_info_rx: mrx
        }
    }
}

impl Service<SessionRequest> for CursorClient {
    type Response = <SessionClient as Service<SessionRequest>>::Response;
    type Error = <SessionClient as Service<SessionRequest>>::Error;
    type Future = <SessionClient as Service<SessionRequest>>::Future;

    fn poll_ready(&mut self, cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        if self.last_block_rx.borrow().is_some()
            && self.first_block_rx.borrow().is_some()
            && self.masterchain_info_rx.borrow().is_some() {
            return self.client.poll_ready(cx)
        }

        cx.waker().wake_by_ref();

        Poll::Pending
    }

    fn call(&mut self, req: SessionRequest) -> Self::Future {
        match req {
            SessionRequest::GetMasterchainInfo {} => {
                let masterchain_info = self.masterchain_info_rx.borrow().as_ref().unwrap().clone();
                async {
                    Ok(serde_json::to_value(masterchain_info)?)
                }.boxed()
            },
            _ => self.client.call(req).boxed()
        }
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
