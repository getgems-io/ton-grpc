use std::borrow::Cow;
use std::convert::Infallible;
use std::future::Future;
use std::pin::Pin;
use std::sync::{Arc, OnceLock};
use std::task::{Context, Poll};
use std::time::Duration;
use tower::{Service, ServiceExt};
use anyhow::Result;
use dashmap::{DashMap, DashSet};
use futures::{FutureExt,try_join, TryFutureExt};
use futures::future::ready;
use futures::never::Never;
use tokio::sync::watch::{Receiver, Sender};
use tokio::time::{interval, MissedTickBehavior};
use tokio_retry::Retry;
use tokio_retry::strategy::{FibonacciBackoff, jitter};
use tower::limit::ConcurrencyLimit;
use tower::load::peak_ewma::Cost;
use tower::load::PeakEwma;
use tower::load::Load;
use tracing::{info, instrument, trace};
use metrics::{absolute_counter, describe_counter, describe_gauge, gauge};
use quick_cache::sync::Cache;
use crate::router::BlockCriteria;
use crate::block::{BlockIdExt, BlocksGetShards, BlocksShards, Sync};
use crate::block::{BlockHeader, BlockId, BlocksLookupBlock, BlocksGetBlockHeader, GetMasterchainInfo, MasterchainInfo};
use crate::client::Client;
use crate::metric::ConcurrencyMetric;
use crate::request::{Specialized, Callable};
use crate::shared::SharedService;

pub type InnerClient = ConcurrencyMetric<ConcurrencyLimit<SharedService<PeakEwma<Client>>>>;

type ChainId = i32;
type ShardId = (i32, i64);
type Seqno = i32;
#[derive(Debug, Clone, Default)]
struct ShardBounds {
    left: Option<BlockHeader>,
    right: Option<BlockHeader>
}

impl ShardBounds {
    fn left(left: BlockHeader) -> Self {
        Self {
            left: Some(left),
            right: None
        }
    }

    fn right(right: BlockHeader) -> Self {
        Self {
            left: None,
            right: Some(right)
        }
    }

    fn distance_seqno(&self, seqno: Seqno) -> Option<Seqno> {
        let left = self.left.as_ref()?;
        let right = self.right.as_ref()?;

        if seqno < left.id.seqno {
            Some(seqno - left.id.seqno)
        } else if seqno > right.id.seqno {
            Some(seqno - right.id.seqno)
        } else {
            Some(0)
        }
    }

    fn distance_lt(&self, lt: i64) -> Option<i64> {
        let left = self.left.as_ref()?;
        let right = self.right.as_ref()?;

        if lt < left.start_lt {
            Some(lt - left.start_lt) // negative
        } else if lt > right.end_lt {
            Some(lt - right.end_lt) // positive
        } else {
            Some(0)
        }
    }

    fn delta_lt(&self) -> Option<i64> {
        let right = self.right.as_ref()?;

        Some(right.end_lt - right.start_lt)
    }
}

type ShardRegistry = DashMap<ChainId, DashSet<ShardId>>;
type ShardBoundsRegistry = DashMap<ShardId, ShardBounds>;

#[derive(Default)]
struct Registry {
    shard_registry: ShardRegistry,
    shard_bounds_registry: ShardBoundsRegistry
}

impl Registry {
    fn get_last_seqno(&self, shard_id: &ShardId) -> Option<Seqno> {
        self.shard_bounds_registry
            .get(shard_id)
            .and_then(|s| s.right.as_ref().map(|h| h.id.seqno))
    }

    fn upsert_left(&self, header: &BlockHeader) {
        let shard_id = (header.id.workchain, header.id.shard);

        self.update_shard_registry(&shard_id);

        trace!(chaid_id = header.id.workchain, shard_id = header.id.shard, seqno = header.id.seqno, "left block");

        self.shard_bounds_registry
            .entry(shard_id)
            .and_modify(|b| { b.left.replace(header.clone()); })
            .or_insert_with(|| ShardBounds::left(header.clone()));
    }

    fn upsert_right(&self, header: &BlockHeader) {
        let shard_id = (header.id.workchain, header.id.shard);

        self.update_shard_registry(&shard_id);

        trace!(chaid_id = header.id.workchain, shard_id = header.id.shard, seqno = header.id.seqno, "right block");

        self.shard_bounds_registry
            .entry(shard_id)
            .and_modify(|b| { b.right.replace(header.clone()); })
            .or_insert_with(|| ShardBounds::right(header.clone()));
    }

    fn update_shard_registry(&self, shard_id: &ShardId) {
        let entry = self.shard_registry
            .entry(shard_id.0)
            .or_default();

        if entry.contains(shard_id) {
            return
        }

        trace!(chaid_id = shard_id.0, shard_id = shard_id.1, "new shard");

        entry.insert(*shard_id);
    }

    fn waitable_distance(&self, chain: &ChainId, criteria: &BlockCriteria) -> Option<Seqno> {
        match criteria {
            BlockCriteria::LogicalTime(lt) => {
                self.shard_registry
                    .get(chain)
                    .and_then(|shard_ids| {
                        let bounds = shard_ids.iter()
                            .filter_map(|shard_id| self.shard_bounds_registry.get(&shard_id));

                        let mut min_waitable_distance = None;
                        let mut delta_lt = None;
                        for bound in bounds {
                            let Some(distance) = bound.distance_lt(*lt) else {
                                continue;
                            };

                            if delta_lt.is_none() {
                                if let Some(new_delta_lt) = bound.delta_lt() {
                                    delta_lt.replace(new_delta_lt);
                                }
                            }

                            if distance == 0 {
                                return Some(0);
                            } else if distance > 0 && distance < *min_waitable_distance.get_or_insert(distance) {
                                min_waitable_distance.replace(distance);
                            }
                        }

                        min_waitable_distance.zip(delta_lt)
                            .map(|(lt_diff, lt_delta)| (lt_delta as f64 / lt_diff as f64).ceil() as Seqno)
                    })
            },

            BlockCriteria::Seqno { shard, seqno} => {
                let shard_id = (*chain, *shard);
                let bounds = self.shard_bounds_registry.get(&shard_id)?;

                let left = bounds.left.as_ref()?;
                let right = bounds.right.as_ref()?;

                let right_lt = right.end_lt - right.start_lt;
                let left_lt = left.end_lt - left.start_lt;

                info!(left_lt = left_lt, right_lt = right_lt, left_start = left.start_lt, right_end = right.end_lt,
                    left_seqno = left.id.seqno, right_seqno = right.id.seqno,
                    "waitable distance");

                bounds.distance_seqno(*seqno).filter(|d| *d >= 0)
            }
        }
    }

    pub fn edges_defined(&self, shard_id: &ShardId) -> bool {
        let Some(shard_bounds) = self.shard_bounds_registry.get(shard_id) else { return false };

        shard_bounds.left.is_some()
    }
}

#[derive(Clone)]
pub struct CursorClient {
    id: Cow<'static, str>,
    client: InnerClient,

    masterchain_info_rx: Receiver<Option<MasterchainInfo>>,
    registry: Arc<Registry>
}

impl CursorClient {
    pub fn last_seqno(&self) -> Option<Seqno> {
        let master_shard_id = self.masterchain_info_rx
            .borrow()
            .as_ref()
            .map(|info| (info.last.workchain, info.last.shard))?;

        self.registry.get_last_seqno(&master_shard_id)
    }

    pub(crate) fn contains(&self, chain: &ChainId, criteria: &BlockCriteria) -> Option<Seqno> {
        let Some(distance) = self.registry.waitable_distance(chain, criteria) else {
            return None;
        };

        if distance > 0 {
            info!(min_waitable_distance = distance);

            return Some(distance);
        };

        Some(0)
    }

    pub fn edges_defined(&self) -> bool {
        let Some(master_shard_id) = self.masterchain_info_rx
            .borrow()
            .as_ref()
            .map(|info| (info.last.workchain, info.last.shard)) else { return false };

        self.registry.edges_defined(&master_shard_id)
    }

    pub fn new(id: String, client: ConcurrencyLimit<SharedService<PeakEwma<Client>>>) -> Self {
        describe_counter!("ton_liteserver_last_seqno", "The seqno of the latest block that is available for the liteserver to sync");
        describe_counter!("ton_liteserver_synced_seqno", "The seqno of the last block with which the liteserver is actually synchronized");
        describe_counter!("ton_liteserver_first_seqno", "The seqno of the first block that is available for the liteserver to request");
        describe_gauge!("ton_liteserver_requests_total", "Total count of requests");
        describe_gauge!("ton_liteserver_requests", "Number of concurrent requests");

        let id = Cow::from(id);
        let client = ConcurrencyMetric::new(client, id.clone());
        let (mtx, mrx) = tokio::sync::watch::channel(None);

        let _self = Self {
            id,
            client,

            masterchain_info_rx: mrx,
            registry: Default::default()
        };

        tokio::spawn(_self.last_block_loop(mtx));
        tokio::spawn(_self.first_block_loop());

        _self
    }

    fn last_block_loop(&self, mtx: Sender<Option<MasterchainInfo>>) -> impl Future<Output = Infallible> {
        let id = self.id.clone();
        let client = self.client.clone();
        let registry = self.registry.clone();

        let discover = LastBlockDiscover { id, client, mtx, registry, current: None };

        discover.discover()
    }

    fn first_block_loop(&self) -> impl Future<Output = Infallible> {
        let id = self.id.clone();
        let client = self.client.clone();
        let registry = self.registry.clone();

        let discover = FirstBlockDiscover::new(id, client, registry, self.masterchain_info_rx.clone());

        discover.discover()
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
        if self.edges_defined() {
            return Service::<GetMasterchainInfo>::poll_ready(&mut self.client, cx);
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

async fn check_block_available(client: &mut InnerClient, block_id: BlockId) -> Result<(BlockHeader, Vec<BlockHeader>)> {
    let block_id = cached_block_id_ext(block_id, client).await?;
    let shards = cached_get_shards(&BlocksGetShards::new(block_id.clone()), client).await?;

    let clone = client.clone();
    let requests = shards.shards
        .into_iter()
        .map(BlocksGetBlockHeader::new)
        .map(|r| clone.clone().oneshot(r));

    try_join!(
        client.oneshot(BlocksGetBlockHeader::new(block_id)),
        futures::future::try_join_all(requests)
    )
}

#[instrument(skip_all, err, level = "trace")]
async fn find_first_blocks(client: &mut InnerClient, start: &BlockIdExt, lhs: Option<i32>, cur: Option<i32>) -> Result<(BlockHeader, Vec<BlockHeader>)> {
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

async fn fetch_last_headers(client: &mut InnerClient) -> Result<(BlockHeader, Vec<BlockHeader>)> {
    let master_chain_last_block_id = client.oneshot(Sync::default()).await?;

    let shards = cached_get_shards(&BlocksGetShards::new(master_chain_last_block_id.clone()), client).await?;

    let clone = client.clone();
    let requests = shards.shards
        .into_iter()
        .map(|s| wait_for_block_header(s, clone.clone()));

    let masterchain_header = BlocksGetBlockHeader::new(master_chain_last_block_id);

    Ok(try_join!(
        cached_block_header(&masterchain_header, client),
        futures::future::try_join_all(requests)
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

async fn wait_for_block_header(block_id: BlockIdExt, client: InnerClient) -> Result<BlockHeader> {
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

struct FirstBlockDiscover {
    id: Cow<'static, str>,
    client: InnerClient,
    registry: Arc<Registry>,
    rx: Receiver<Option<MasterchainInfo>>,
    current: Option<BlockHeader>,
}

impl FirstBlockDiscover {
    fn new(id: Cow<'static, str>, client: InnerClient, registry: Arc<Registry>, rx: Receiver<Option<MasterchainInfo>>) -> Self {
        Self {
            id,
            client,
            registry,
            rx,
            current: None
        }
    }

    async fn discover(mut self) -> Never {
        let mut timer = interval(Duration::from_secs(30));
        timer.set_missed_tick_behavior(MissedTickBehavior::Skip);

        loop {
            timer.tick().await;

            let Some(start) = self.rx.borrow().as_ref().map(|m| m.last.clone()) else {
                continue;
            };

            match self.next(start).await {
                Ok(Some(mfb)) => { self.current.replace(mfb); }
                Err(_) | Ok(None) => {}
            }
        }
    }

    async fn next(&mut self, start: BlockIdExt) -> Result<Option<BlockHeader>> {
        if let Some(ref mfb) = self.current {
            if let Err(e) = (&mut self.client).oneshot(BlocksGetShards::new(mfb.id.clone())).await {
                trace!(seqno = mfb.id.seqno, e = ?e, "first block not available anymore");
            } else {
                trace!("first block still available");

                return Ok(None);
            }
        }

        let lhs = self.current.as_ref().map(|n| n.id.seqno + 1);
        let cur = self.current.as_ref().map(|n| n.id.seqno + 32);
        let (mfb, wfb) = find_first_blocks(&mut self.client, &start, lhs, cur).await?;

        absolute_counter!("ton_liteserver_first_seqno", mfb.id.seqno as u64, "liteserver_id" => self.id.clone());

        self.registry.upsert_left(&mfb);
        for header in &wfb {
            self.registry.upsert_left(header);
        }

        Ok(Some(mfb))
    }
}

struct LastBlockDiscover {
    id: Cow<'static, str>,
    client: InnerClient,
    registry: Arc<Registry>,
    current: Option<MasterchainInfo>,
    mtx: Sender<Option<MasterchainInfo>>
}

impl LastBlockDiscover {
    async fn discover(mut self) -> Never {
        let mut timer = interval(Duration::new(2, 1_000_000_000 / 2));
        timer.set_missed_tick_behavior(MissedTickBehavior::Skip);

        let mut current: Option<MasterchainInfo> = None;
        loop {
            timer.tick().await;

            gauge!("ton_liteserver_requests", self.client.load() as f64, "liteserver_id" => self.id.clone());

            match self.next().await {
                Ok(Some(masterchain_info)) => { current.replace(masterchain_info); },
                Ok(None) | Err(_) => {}
            }
        }
    }

    async fn next(&mut self) -> Result<Option<MasterchainInfo>> {
        let mut masterchain_info = (&mut self.client).oneshot(GetMasterchainInfo::default()).await?;
        if let Some(ref current) = self.current {
            if current == &masterchain_info {
                return Ok(None);
            }
        }

        absolute_counter!("ton_liteserver_last_seqno", masterchain_info.last.seqno as u64, "liteserver_id" => self.id.clone());

        let (master_header, last_work_chain_header) = fetch_last_headers(&mut self.client).await?;
        absolute_counter!("ton_liteserver_synced_seqno", master_header.id.seqno as u64, "liteserver_id" => self.id.clone());

        self.registry.upsert_right(&master_header);
        for header in &last_work_chain_header {
            self.registry.upsert_right(header);
        }

        masterchain_info.last = master_header.id.clone();
        let _ = self.mtx.send(Some(masterchain_info.clone()));

        Ok(Some(masterchain_info))
    }
}
