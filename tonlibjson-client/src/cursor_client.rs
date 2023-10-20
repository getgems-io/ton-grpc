use std::borrow::Cow;
use std::future::Future;
use std::pin::Pin;
use std::sync::OnceLock;
use std::task::{Context, Poll};
use std::time::Duration;
use tower::{Service, ServiceExt};
use anyhow::{anyhow, Result};
use futures::{FutureExt, try_join, TryFutureExt};
use futures::future::ready;
use tokio::sync::watch::Receiver;
use tokio::time::{Instant, interval, MissedTickBehavior};
use tokio_retry::Retry;
use tokio_retry::strategy::{FibonacciBackoff, jitter};
use tower::limit::ConcurrencyLimit;
use tower::load::peak_ewma::Cost;
use tower::load::PeakEwma;
use tower::load::Load;
use tracing::{instrument, trace};
use metrics::{absolute_counter, describe_counter, describe_gauge, gauge};
use quick_cache::sync::Cache;
use crate::block::{BlockIdExt, BlocksGetShards, BlocksShards, Sync};
use crate::block::{BlockHeader, BlockId, BlocksLookupBlock, BlocksGetBlockHeader, GetMasterchainInfo, MasterchainInfo};
use crate::client::Client;
use crate::metric::ConcurrencyMetric;
use crate::request::{Specialized, Callable};
use crate::shared::SharedService;

pub type InnerClient = ConcurrencyMetric<ConcurrencyLimit<SharedService<PeakEwma<Client>>>>;

#[derive(Clone)]
pub struct CursorClient {
    client: InnerClient,

    pub first_block_rx: Receiver<Option<(BlockHeader, BlockHeader)>>,
    pub last_block_rx: Receiver<Option<(BlockHeader, BlockHeader)>>,

    masterchain_info_rx: Receiver<Option<MasterchainInfo>>
}

impl CursorClient {
    pub fn new(id: String, client: ConcurrencyLimit<SharedService<PeakEwma<Client>>>) -> Self {
        describe_counter!("ton_liteserver_last_seqno", "The seqno of the latest block that is available for the liteserver to sync");
        describe_counter!("ton_liteserver_synced_seqno", "The seqno of the last block with which the liteserver is actually synchronized");
        describe_counter!("ton_liteserver_first_seqno", "The seqno of the first block that is available for the liteserver to request");
        describe_gauge!("ton_liteserver_requests_total", "Total count of requests");
        describe_gauge!("ton_liteserver_requests", "Number of concurrent requests");

        let id = Cow::from(id);
        let client = ConcurrencyMetric::new(client, id.clone());

        let (ctx, crx) = tokio::sync::watch::channel(None);
        let (mtx, mrx) = tokio::sync::watch::channel(None);
        tokio::spawn({
            let id = id.clone();
            let mut client = client.clone();
            async move {
                let mut timer = interval(Duration::new(2, 1_000_000_000 / 2));
                timer.set_missed_tick_behavior(MissedTickBehavior::Skip);

                let mut current: Option<MasterchainInfo> = None;
                loop {
                    timer.tick().await;

                    gauge!("ton_liteserver_requests", client.load() as f64, "liteserver_id" => id.clone());

                    let masterchain_info = (&mut client).oneshot(GetMasterchainInfo::default()).await;

                    match masterchain_info {
                        Ok(mut masterchain_info) => {
                            if let Some(cur) = current.clone() {
                                if cur == masterchain_info {
                                    trace!(cursor = cur.last.seqno, "block actual");

                                    continue;
                                } else {
                                    trace!(cursor = cur.last.seqno, actual = masterchain_info.last.seqno, "block discovered");
                                    absolute_counter!("ton_liteserver_last_seqno", cur.last.seqno as u64, "liteserver_id" => id.clone());
                                }
                            }

                            match fetch_last_headers(&mut client).await {
                                Ok((last_master_chain_header, last_work_chain_header)) => {
                                    masterchain_info.last = last_master_chain_header.id.clone();
                                    absolute_counter!("ton_liteserver_synced_seqno", last_master_chain_header.id.seqno as u64, "liteserver_id" => id.clone());
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
            let mut client = client.clone();
            let mut first_block: Option<(BlockHeader, BlockHeader)> = None;
            let mut first_block_seqno = None;

            async move {
                let mut timer = interval(Duration::from_secs(30));
                timer.set_missed_tick_behavior(MissedTickBehavior::Skip);

                loop {
                    let Ok(start) = (&mut client).oneshot(GetMasterchainInfo::default()).await else {
                        timer.tick().await;
                        continue;
                    };

                    let start_time = Instant::now();
                    while start_time.elapsed() < Duration::from_secs(60 * 60 * 4) { // Every 4 hours reset starting point
                        timer.tick().await;

                        if let Some((mfb, _)) = &first_block {
                            if let Err(e) = (&mut client).oneshot(BlocksGetShards::new(mfb.id.clone())).await {
                                trace!(seqno = mfb.id.seqno, e = ?e, "first block not available anymore");
                                first_block = None;
                            } else {
                                trace!("first block still available");
                                continue;
                            }
                        }

                        let fb = find_first_blocks(&mut client, &start.last, first_block_seqno.map(|n| n + 1), first_block_seqno.map(|n| n + 32)).await;
                        match fb {
                            Ok((mfb, wfb)) => {
                                absolute_counter!("ton_liteserver_first_seqno", mfb.id.seqno as u64, "liteserver_id" => id.clone());
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

// TODO[akostylev0] generics
impl Service<Specialized<BlocksGetShards>> for CursorClient {
    type Response = BlocksShards;
    type Error = anyhow::Error;
    type Future = Pin<Box<dyn Future<Output = Result<Self::Response, Self::Error>> + Send>>;

    fn poll_ready(&mut self, cx: &mut Context<'_>) -> Poll<std::result::Result<(), Self::Error>> {
        Service::<BlocksGetShards>::poll_ready(self, cx)
    }

    fn call(&mut self, req: Specialized<BlocksGetShards>) -> Self::Future {
        let mut client = self.client.clone();

        async move { cached_get_shards(req.inner(), &mut client).await }.boxed()
    }
}

// TODO[akostylev0] generics
impl Service<Specialized<BlocksLookupBlock>> for CursorClient {
    type Response = BlockIdExt;
    type Error = anyhow::Error;
    type Future = Pin<Box<dyn Future<Output = Result<Self::Response, Self::Error>> + Send>>;

    fn poll_ready(&mut self, cx: &mut Context<'_>) -> Poll<std::result::Result<(), Self::Error>> {
        Service::<BlocksLookupBlock>::poll_ready(self, cx)
    }

    fn call(&mut self, req: Specialized<BlocksLookupBlock>) -> Self::Future {
        let mut client = self.client.clone();

        async move { cached_lookup_block(req.inner(), &mut client).await }.boxed()
    }
}

impl<R : Callable<InnerClient>> Service<R> for CursorClient {
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

impl Load for CursorClient {
    type Metric = Cost;

    fn load(&self) -> Self::Metric {
        self.client.get_ref().load()
    }
}

async fn check_block_available(client: &mut InnerClient, block_id: BlockId) -> Result<(BlockHeader, BlockHeader)> {
    let block_id = cached_block_id_ext(block_id, client).await?;
    let shards = cached_get_shards(&BlocksGetShards::new(block_id.clone()), client).await?;

    try_join!(
        client.clone().oneshot(BlocksGetBlockHeader::new(block_id)),
        client.oneshot(BlocksGetBlockHeader::new(shards.shards.first().expect("must be exist").clone()))
    )
}

#[instrument(skip_all, err, level = "trace")]
async fn find_first_blocks(client: &mut InnerClient, start: &BlockIdExt, lhs: Option<i32>, cur: Option<i32>) -> Result<(BlockHeader, BlockHeader)> {
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

async fn fetch_last_headers(client: &mut InnerClient) -> Result<(BlockHeader, BlockHeader)> {
    let master_chain_last_block_id = client.oneshot(Sync::default()).await?;

    let shards = cached_get_shards(&BlocksGetShards::new(master_chain_last_block_id.clone()), client).await?;
    // TODO[akostylev0] handle case when there are multiple shards
    let work_chain_last_block_id = shards.shards.first()
        .ok_or_else(|| anyhow!("last block for work chain not found"))?
        .clone();

    let mut clone = client.clone();
    let masterchain_header = BlocksGetBlockHeader::new(master_chain_last_block_id);

    Ok(try_join!(
        cached_block_header(&masterchain_header, &mut clone),
        wait_for_block_header(work_chain_last_block_id, client)
    )?)
}


fn shards_cache() -> &'static Cache<BlockIdExt, BlocksShards> {
    static CACHE: OnceLock<Cache<BlockIdExt, BlocksShards>> = OnceLock::new();

    CACHE.get_or_init(|| Cache::new(1024))
}
async fn cached_get_shards(req: &BlocksGetShards, client: &mut InnerClient) -> Result<BlocksShards> {
    let key = req.id.clone();

    shards_cache().get_or_insert_async(&key, async { client.oneshot(req.clone()).await }).await
}

fn block_cache() -> &'static Cache<BlocksLookupBlock, BlockIdExt> {
    static CACHE: OnceLock<Cache<BlocksLookupBlock, BlockIdExt>> = OnceLock::new();

    CACHE.get_or_init(|| Cache::new(1024))
}

async fn cached_block_id_ext(block_id: BlockId, client: &mut InnerClient) -> Result<BlockIdExt> {
    let req = BlocksLookupBlock::seqno(block_id);

    cached_lookup_block(&req, client).await
}

async fn cached_lookup_block(req: &BlocksLookupBlock, client: &mut InnerClient) -> Result<BlockIdExt> {
    block_cache().get_or_insert_async(req, async { client.oneshot(req.clone()).await }).await
}

fn block_header_cache() -> &'static Cache<BlocksGetBlockHeader, BlockHeader> {
    static CACHE: OnceLock<Cache<BlocksGetBlockHeader, BlockHeader>> = OnceLock::new();

    CACHE.get_or_init(|| Cache::new(1024))
}

async fn cached_block_header(req: &BlocksGetBlockHeader, client: &mut InnerClient) -> Result<BlockHeader> {
    block_header_cache().get_or_insert_async(req, async { client.oneshot(req.clone()).await }).await
}

async fn wait_for_block_header(block_id: BlockIdExt, client: &mut InnerClient) -> Result<BlockHeader> {
    let retry = FibonacciBackoff::from_millis(512)
        .max_delay(Duration::from_millis(4096))
        .map(jitter)
        .take(16);

    Retry::spawn(retry, || {
        let block_id = block_id.clone();
        let client = client.clone();

        client.oneshot(BlocksGetBlockHeader::new(block_id))
    }).await
}
