use std::future::Future;
use std::pin::Pin;
use std::task::{Context, Poll};
use std::time::Duration;
use tower::{Service, ServiceExt};
use anyhow::{anyhow, Result};
use futures::{FutureExt, try_join, TryFutureExt};
use futures::future::ready;
use tokio::sync::watch::Receiver;
use tokio::time::{interval, MissedTickBehavior};
use tokio_retry::Retry;
use tokio_retry::strategy::{ExponentialBackoff, jitter};
use tower::limit::ConcurrencyLimit;
use tower::load::peak_ewma::Cost;
use tower::load::PeakEwma;
use tracing::{instrument, trace};
use metrics::{absolute_counter, describe_counter};
use crate::block::{BlockIdExt, BlocksGetShards, Sync};
use crate::block::{BlockHeader, BlockId, BlocksLookupBlock, BlocksGetBlockHeader, GetMasterchainInfo, MasterchainInfo};
use crate::client::Client;
use crate::request::{Specialized, Callable};
use crate::shared::SharedService;

#[derive(Clone)]
pub struct CursorClient {
    client: ConcurrencyLimit<SharedService<PeakEwma<Client>>>,

    pub first_block_rx: Receiver<Option<(BlockHeader, BlockHeader)>>,
    pub last_block_rx: Receiver<Option<(BlockHeader, BlockHeader)>>,

    masterchain_info_rx: Receiver<Option<MasterchainInfo>>
}

impl CursorClient {
    pub fn new(id: String, client: ConcurrencyLimit<SharedService<PeakEwma<Client>>>) -> Self {
        let labels = [("liteserver_id", format!("{}!", id))];
        describe_counter!("ton_liteserver_last_seqno", "The seqno of the latest block that is available for the liteserver to sync");
        describe_counter!("ton_liteserver_synced_seqno", "The seqno of the last block with which the liteserver is actually synchronized");
        describe_counter!("ton_liteserver_first_seqno", "The seqno of the first block that is available for the liteserver to request");

        let (ctx, crx) = tokio::sync::watch::channel(None);
        let (mtx, mrx) = tokio::sync::watch::channel(None);
        tokio::spawn({
            let labels = labels.clone();
            let mut client = client.clone();
            async move {
                let mut timer = interval(Duration::new(2, 1_000_000_000 / 2));
                timer.set_missed_tick_behavior(MissedTickBehavior::Skip);

                let mut current: Option<MasterchainInfo> = None;
                loop {
                    timer.tick().await;

                    let masterchain_info = (&mut client).oneshot(GetMasterchainInfo::default()).await;

                    match masterchain_info {
                        Ok(mut masterchain_info) => {
                            if let Some(cur) = current.clone() {
                                if cur == masterchain_info {
                                    trace!(cursor = cur.last.seqno, "block actual");

                                    continue;
                                } else {
                                    trace!(cursor = cur.last.seqno, actual = masterchain_info.last.seqno, "block discovered");
                                    absolute_counter!("ton_liteserver_last_seqno", cur.last.seqno as u64, &labels);
                                }
                            }

                            match fetch_last_headers(&mut client).await {
                                Ok((last_master_chain_header, last_work_chain_header)) => {
                                    masterchain_info.last = last_master_chain_header.id.clone();
                                    absolute_counter!("ton_liteserver_synced_seqno", last_master_chain_header.id.seqno as u64, &labels);
                                    trace!(seqno = last_master_chain_header.id.seqno, "master chain block reached");
                                    trace!(seqno = last_work_chain_header.id.seqno, "work chain block reached");

                                    current.replace(masterchain_info.clone());

                                    let _ = mtx.send(Some(masterchain_info));
                                    let _ = ctx.send(Some((last_master_chain_header, last_work_chain_header)));
                                },
                                Err(e) => trace!(e = ?e, "unable to fetch last headers")
                            }
                        },
                        Err(e) => trace!(e = ?e, "unable to get master chain info")
                    }
                }
            }
        });

        let (ftx, frx) = tokio::sync::watch::channel(None);
        tokio::spawn({
            let labels = labels.clone();
            let mut client = client.clone();
            let mut first_block: Option<(BlockHeader, BlockHeader)> = None;
            let mut first_block_seqno = None;

            async move {
                let mut timer = interval(Duration::from_secs(30));
                timer.set_missed_tick_behavior(MissedTickBehavior::Skip);
                loop {
                    timer.tick().await;

                    if let Some((mfb, wfb)) = first_block.clone() {
                        if let Err(e) = try_join!(
                            client.clone().oneshot(BlocksGetShards::new(mfb.id.clone())),
                            (&mut client).oneshot(BlocksGetBlockHeader::new(wfb.id.clone()))
                        ) {
                            trace!(seqno = mfb.id.seqno, e = ?e, "first block not available anymore");
                            first_block = None;
                        } else {
                            trace!("first block still available");
                        }
                    }
                    if first_block.is_none() {
                        let fb = find_first_blocks(&mut client, first_block_seqno.map(|n| n + 1), first_block_seqno.map(|n| n + 32)).await;

                        match fb {
                            Ok((mfb, wfb)) => {
                                absolute_counter!("ton_liteserver_first_seqno", mfb.id.seqno as u64, &labels);
                                trace!(seqno = mfb.id.seqno, "master chain first block");
                                trace!(seqno = wfb.id.seqno, "work chain first block");

                                first_block = Some((mfb.clone(), wfb.clone()));
                                first_block_seqno = Some(mfb.id.seqno);

                                let _ = ftx.send(Some((mfb, wfb)));
                            },
                            Err(e) => trace!(e = ?e, "unable to fetch first headers")
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

    pub fn headers(&self, chain_id: i32) -> Option<(BlockHeader, BlockHeader)> {
        let Some(first_block) = self.first_block_rx.borrow().clone() else {
            return None;
        };
        let Some(last_block) = self.last_block_rx.borrow().clone() else {
            return None;
        };

        match chain_id {
            -1 => Some((first_block.0, last_block.0)),
            _ => Some((first_block.1, last_block.1))
        }
    }
}

impl Service<Specialized<GetMasterchainInfo>> for CursorClient {
    type Response = MasterchainInfo;
    type Error = anyhow::Error;
    type Future = Pin<Box<dyn Future<Output = Result<Self::Response, Self::Error>> + Send>>;

    fn poll_ready(&mut self, cx: &mut Context<'_>) -> Poll<std::result::Result<(), Self::Error>> {
        if self.masterchain_info_rx.borrow().is_some() {
            Poll::Ready(Ok(()))
        } else {
            cx.waker().wake_by_ref();

            Poll::Pending
        }
    }

    fn call(&mut self, _: Specialized<GetMasterchainInfo>) -> Self::Future {
        let response = self.masterchain_info_rx.borrow().as_ref().unwrap().clone();

        return ready(Ok(response)).boxed()
    }
}

impl<R : Callable<ConcurrencyLimit<SharedService<PeakEwma<Client>>>>> Service<R> for CursorClient {
    type Response = R::Response;
    type Error = anyhow::Error;
    type Future = Pin<Box<dyn Future<Output = Result<Self::Response, Self::Error>> + Send>>;

    fn poll_ready(&mut self, cx: &mut Context<'_>) -> Poll<std::result::Result<(), Self::Error>> {
        if self.last_block_rx.borrow().is_some()
            && self.first_block_rx.borrow().is_some()
            && self.masterchain_info_rx.borrow().is_some() {
            return Service::<GetMasterchainInfo>::poll_ready(&mut self.client, cx)
        }

        cx.waker().wake_by_ref();

        Poll::Pending
    }

    fn call(&mut self, req: R) -> Self::Future {
        req.call(&mut self.client).map_err(|e| e.into().into()).boxed()
    }
}

impl tower::load::Load for CursorClient {
    type Metric = Cost;

    fn load(&self) -> Self::Metric {
        self.client.load()
    }
}

async fn check_block_available(client: &mut ConcurrencyLimit<SharedService<PeakEwma<Client>>>, block_id: BlockId) -> Result<(BlockHeader, BlockHeader)> {
    let block_id = client.oneshot(BlocksLookupBlock::seqno(block_id)).await?;
    let shards = client.oneshot(BlocksGetShards::new(block_id.clone())).await?;

    try_join!(
        client.clone().oneshot(BlocksGetBlockHeader::new(block_id)),
        client.oneshot(BlocksGetBlockHeader::new(shards.shards.first().expect("must be exist").clone()))
    )
}

#[instrument(skip_all, err, level = "trace")]
async fn find_first_blocks(client: &mut ConcurrencyLimit<SharedService<PeakEwma<Client>>>, lhs: Option<i32>, cur: Option<i32>) -> Result<(BlockHeader, BlockHeader)> {
    let start = client.oneshot(GetMasterchainInfo::default()).await?.last;

    let length = start.seqno;
    let mut rhs = length;
    let mut lhs = lhs.unwrap_or(1);
    let mut cur = cur.unwrap_or(start.seqno - 200000);

    let workchain = start.workchain;
    let shard = start.shard;

    let mut block = check_block_available(client, BlockId::new(workchain, shard, cur)).await;
    let mut success = None;

    let mut hops = 0;

    while lhs < rhs {
        // TODO[akostylev0] specify error
        if block.is_err() {
            lhs = cur + 1;
        } else {
            rhs = cur;
        }

        cur = (lhs + rhs) / 2;
        if cur == 0 { break; }

        block = check_block_available(client, BlockId::new(workchain, shard, cur)).await;
        if block.is_ok() {
            success = Some(block.as_ref().unwrap().clone());
        }

        hops += 1;
    }

    let delta = 4;
    let (master, work) = match block {
        Ok(b) => { b },
        Err(e) => {
            match success {
                Some(b) if b.0.id.seqno - cur <= delta => { b },
                _ => { return Err(e) },
            }
        }
    };

    trace!(hops = hops, seqno = master.id.seqno, "first seqno");

    Ok((master, work))
}


async fn fetch_last_headers(client: &mut ConcurrencyLimit<SharedService<PeakEwma<Client>>>) -> Result<(BlockHeader, BlockHeader)> {
    let master_chain_last_block_id = client.oneshot(Sync::default()).await?;

    let shards = client.oneshot(BlocksGetShards::new(master_chain_last_block_id.clone()))
        .await?.shards;

    let work_chain_last_block_id = shards.first()
        .ok_or_else(|| anyhow!("last block for work chain not found"))?
        .clone();

    let (master_chain_header, work_chain_header) = try_join!(
        client.clone().oneshot(BlocksGetBlockHeader::new(master_chain_last_block_id)),
        wait_for_block_header(work_chain_last_block_id, client)
    )?;

    Ok((master_chain_header, work_chain_header))
}

async fn wait_for_block_header(block_id: BlockIdExt, client: &mut ConcurrencyLimit<SharedService<PeakEwma<Client>>>) -> Result<BlockHeader> {
    let retry = ExponentialBackoff::from_millis(4)
        .max_delay(Duration::from_secs(1))
        .map(jitter)
        .take(16);

    Retry::spawn(retry, || {
        let block_id = block_id.clone();
        let client = client.clone();

        client.oneshot(BlocksGetBlockHeader::new(block_id))
    }).await
}
