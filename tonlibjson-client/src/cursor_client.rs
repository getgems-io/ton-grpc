use std::borrow::BorrowMut;
use std::cmp::Ordering;
use std::future::Future;
use std::process::Output;
use std::task::{Context, Poll};
use std::time::Duration;
use tower::{Service, ServiceExt};
use anyhow::{anyhow, Result};
use derive_new::new;
use futures::{FutureExt, TryFutureExt};
use futures::future::BoxFuture;
use tokio::sync::watch::Receiver;
use tokio::task::JoinHandle;
use tokio::time::{interval, MissedTickBehavior};
use tower::limit::ConcurrencyLimit;
use tower::load::peak_ewma::Cost;
use tracing::{debug, error, trace};
use crate::block::{BlockHeader, BlockId, BlockIdExt, BlocksGetShards, BlocksLookupBlock, GetBlockHeader, GetMasterchainInfo, MasterchainInfo};
use crate::request::{Request, Requestable};
use crate::session::{SessionClient, SessionRequest};

pub const MAINCHAIN_ID: i64 = -1;

#[derive(new, Clone)]
pub struct ChainState {
    pub main_chain: BlockHeader,
    pub work_chain: BlockHeader
}

pub struct CursorClient {
    client: ConcurrencyLimit<SessionClient>,

    first_block_rx: Receiver<Option<ChainState>>,
    last_block_rx: Receiver<Option<ChainState>>,

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

                    let masterchain_info: Result<MasterchainInfo> = GetMasterchainInfo::default()
                        .call(client.borrow_mut())
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

                            let state = synchronize(client.borrow_mut()).await;

                            let last_block: Result<BlockHeader> = client
                                .ready()
                                .and_then(|c| c.call(SessionRequest::Synchronize {}))
                                .map_ok(|val| serde_json::from_value::<BlockHeader>(val).unwrap())
                                .await;

                            match state {
                                Ok(state) => {
                                    masterchain_info.last = state.main_chain.id.clone();
                                    trace!(seqno = state.main_chain.id.seqno, "block reached");

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
            let mut state: Option<ChainState> = None;

            async move {
                let mut timer = interval(Duration::from_secs(30));
                timer.set_missed_tick_behavior(MissedTickBehavior::Skip);
                loop {
                    timer.tick().await;

                    if let Some(fb) = state.clone() {
                        let fb = BlocksLookupBlock::seqno(fb.main_chain.into()).call(client.borrow_mut()).await;
                        if let Err(e) = fb {
                            error!("{}", e);
                            state = None;
                        } else {
                            trace!("first block still available")
                        }
                    }

                    if state.is_none() {
                        let fs = find_first_state(client.borrow_mut(), )
                        let fb = client
                            .ready()
                            .and_then(|c| c.call(SessionRequest::FindFirstBlock { chain_id: MAINCHAIN_ID}))
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

async fn synchronize(client: &mut ConcurrencyLimit<SessionClient>) -> Result<ChainState> {
    let block = Sync::default()
        .call(client.borrow_mut())
        .await?;

    let header_request = GetBlockHeader::new(block.clone());
    let shards_request = BlocksGetShards::new(block.clone());

    let (block_header, shards) = futures::future::try_join(
        header_request.call(client.borrow_mut()),
        shards_request.call(client.borrow_mut())
    ).await?;

    let work_block = shards.shards.first()
        .ok_or_else(|| anyhow!("workchain shard not found"))?
        .to_owned();
    let work_header = GetBlockHeader::new(work_block)
        .call(client.borrow_mut())
        .await?;

    Ok(ChainState::new(block_header, work_header))
}

async fn find_first_state(client: &mut ConcurrencyLimit<SessionClient>, last: BlockIdExt) -> Result<BlockHeader> {
    let length = last.seqno;
    let mut cur = length / 2;
    let mut rhs = length;
    let mut lhs = 1;

    let workchain = last.workchain;
    let shard = last.shard;

    let mut block = BlocksLookupBlock::seqno(BlockId {
        workchain,
        shard,
        seqno: cur
    }).call(client.borrow_mut()).await;

    while lhs < rhs {
        // TODO[akostylev0] specify error
        if block.is_err() {
            lhs = cur + 1;
        } else {
            rhs = cur;
        }

        cur = (lhs + rhs) / 2;

        debug!("lhs: {}, rhs: {}, cur: {}", lhs, rhs, cur);

        block = BlocksLookupBlock::seqno(BlockId { workchain, shard, seqno: cur })
            .call(client.borrow_mut())
            .await;
    }

    GetBlockHeader::new(block?).call(client.borrow_mut()).await
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
