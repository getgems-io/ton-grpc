use std::borrow::Cow;
use std::collections::{HashMap, HashSet};
use std::convert::Infallible;
use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;
use std::task::{Context, Poll};
use std::time::Duration;
use tower::{Service, ServiceExt};
use anyhow::Result;
use dashmap::{DashMap, DashSet};
use futures::{FutureExt, try_join, TryFutureExt};
use futures::future::ready;
use futures::never::Never;
use tokio::sync::mpsc::UnboundedSender;
use tokio::sync::watch::{Receiver, Sender};
use tokio::time::{interval, MissedTickBehavior};
use tokio_retry::Retry;
use tokio_retry::strategy::{FibonacciBackoff, jitter};
use tower::limit::ConcurrencyLimit;
use tower::load::peak_ewma::Cost;
use tower::load::PeakEwma;
use tower::load::Load;
use tracing::{instrument};
use ton_client_util::router::Routed;
use ton_client_util::router::route::BlockCriteria;
use ton_client_util::router::shards::Prefix;
use crate::block::{BlocksGetMasterchainInfo, BlocksGetShards, BlocksHeader, BlocksMasterchainInfo, Sync, TonBlockId, TonBlockIdExt};
use crate::block::{BlocksLookupBlock, BlocksGetBlockHeader};
use crate::client::Client;
use crate::metric::ConcurrencyMetric;
use crate::request::{Specialized, Callable};
use crate::shared::SharedService;

pub(crate) type InnerClient = ConcurrencyMetric<ConcurrencyLimit<SharedService<PeakEwma<Client>>>>;

type ChainId = i32;
type ShardId = (i32, i64);

impl From<&TonBlockIdExt> for ShardId {
    fn from(value: &TonBlockIdExt) -> Self {
        (value.workchain, value.shard)
    }
}

type Seqno = i32;
#[derive(Debug, Clone, Default)]
struct ShardBounds {
    left: Option<BlocksHeader>,
    right: Option<BlocksHeader>,
    right_end: Option<Seqno>
}

impl ShardBounds {
    fn left(left: BlocksHeader) -> Self {
        Self {
            left: Some(left),
            right: None,
            right_end: None
        }
    }

    fn right(right: BlocksHeader) -> Self {
        Self {
            left: None,
            right_end: Some(right.id.seqno),
            right: Some(right),
        }
    }

    fn right_end(right_end: Seqno) -> Self {
        Self {
            left: None,
            right_end: Some(right_end),
            right: None,
        }
    }

    fn right_next(&self) -> Option<Seqno> {
        let seqno = self.right_end?;

        match self.right {
            None => Some(seqno),
            Some(ref right) if right.id.seqno < seqno => Some(right.id.seqno + 1),
            _ => None,
        }
    }

    fn contains_seqno(&self, seqno: Seqno, not_available: bool) -> bool {
        let Some(ref left) = self.left else { return false };
        let Some(ref right) = self.right else { return false };

        if not_available {
            left.id.seqno <= seqno && seqno <= self.right_end.unwrap_or(right.id.seqno)
        } else {
            left.id.seqno <= seqno && seqno <= right.id.seqno
        }
    }

    fn contains_lt(&self, lt: i64, not_available: bool) -> bool {
        let Some(ref left) = self.left else { return false };
        let Some(ref right) = self.right else { return false };

        if not_available {
            left.start_lt <= lt && lt <= right.end_lt + (right.end_lt - right.start_lt)
        } else {
            left.start_lt <= lt && lt <= right.end_lt
        }
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
    fn right_next(&self, shard_id: ShardId) -> Option<Seqno> {
        self.shard_bounds_registry
            .get(&shard_id)
            .and_then(|s| s.right_next())
    }

    fn get_last_seqno(&self, shard_id: &ShardId) -> Option<Seqno> {
        self.shard_bounds_registry
            .get(shard_id)
            .and_then(|s| s.right.as_ref().map(|h| h.id.seqno))
    }

    fn upsert_left(&self, header: &BlocksHeader) {
        let shard_id = (header.id.workchain, header.id.shard);

        self.update_shard_registry(&shard_id);

        tracing::trace!(chaid_id = header.id.workchain, shard_id = header.id.shard, seqno = header.id.seqno, "left block");

        self.shard_bounds_registry
            .entry(shard_id)
            .and_modify(|b| { b.left.replace(header.clone()); })
            .or_insert_with(|| ShardBounds::left(header.clone()));
    }

    fn upsert_right(&self, header: &BlocksHeader) {
        let shard_id = (header.id.workchain, header.id.shard);

        self.update_shard_registry(&shard_id);

        tracing::trace!(chaid_id = header.id.workchain, shard_id = header.id.shard, seqno = header.id.seqno, "right block");

        self.shard_bounds_registry
            .entry(shard_id)
            .and_modify(|b| { b.right.replace(header.clone()); })
            .or_insert_with(|| ShardBounds::right(header.clone()));
    }

    fn upsert_right_end(&self, block_id: &TonBlockIdExt) {
        let shard_id = (block_id.workchain, block_id.shard);

        self.update_shard_registry(&shard_id);

        tracing::trace!(chaid_id = block_id.workchain, shard_id = block_id.shard, seqno = block_id.seqno, "right end block");

        self.shard_bounds_registry
            .entry(shard_id)
            .and_modify(|b| { b.right_end.replace(block_id.seqno); })
            .or_insert_with(|| ShardBounds::right_end(block_id.seqno));
    }

    fn update_shard_registry(&self, shard_id: &ShardId) {
        let entry = self.shard_registry
            .entry(shard_id.0)
            .or_default();

        if entry.contains(shard_id) {
            return
        }

        tracing::trace!(chaid_id = shard_id.0, shard_id = shard_id.1, "new shard");

        entry.insert(*shard_id);
    }

    fn contains(&self, chain: &ChainId, criteria: &BlockCriteria, not_available: bool) -> bool {
        match criteria {
            BlockCriteria::LogicalTime { address, lt } => {
                self.shard_registry
                    .get(chain)
                    .map(|shard_ids| shard_ids
                        .iter()
                        .filter_map(|shard_id|
                            Prefix::from_shard_id(shard_id.1 as u64)
                                .matches(address)
                                .then(|| self.shard_bounds_registry.get(&shard_id))
                                .flatten()
                        )
                        .any(|bounds| bounds.contains_lt(*lt, not_available))
                    ).unwrap_or(false)
            },
            BlockCriteria::Seqno { shard, seqno } => {
                let shard_id = (*chain, *shard);
                let Some(bounds) = self.shard_bounds_registry.get(&shard_id) else {
                    return false
                };

                bounds.contains_seqno(*seqno, not_available)
            }
        }
    }

    pub fn edges_defined(&self, shard_id: &ShardId) -> bool {
        let Some(shard_bounds) = self.shard_bounds_registry.get(shard_id) else { return false };

        shard_bounds.left.is_some()
    }
}

#[derive(Clone)]
pub(crate) struct CursorClient {
    id: Cow<'static, str>,
    client: InnerClient,

    masterchain_info_rx: Receiver<Option<BlocksMasterchainInfo>>,
    registry: Arc<Registry>
}

impl Routed for CursorClient {
    fn contains(&self, chain: &ChainId, criteria: &BlockCriteria) -> bool {
        self.registry.contains(chain, criteria, false)
    }

    fn contains_not_available(&self, chain: &ChainId, criteria: &BlockCriteria) -> bool {
        self.registry.contains(chain, criteria, true)
    }

    fn last_seqno(&self) -> Option<Seqno> {
        let master_shard_id = self.masterchain_info_rx
            .borrow()
            .as_ref()
            .map(|info| (info.last.workchain, info.last.shard))?;

        self.registry.get_last_seqno(&master_shard_id)
    }
}

impl CursorClient {
    pub(crate) fn new(id: String, client: ConcurrencyLimit<SharedService<PeakEwma<Client>>>) -> Self {
        metrics::describe_counter!("ton_liteserver_last_seqno", "The seqno of the latest block that is available for the liteserver to sync");
        metrics::describe_counter!("ton_liteserver_synced_seqno", "The seqno of the last block with which the liteserver is actually synchronized");
        metrics::describe_counter!("ton_liteserver_first_seqno", "The seqno of the first block that is available for the liteserver to request");
        metrics::describe_gauge!("ton_liteserver_requests_total", "Total count of requests");
        metrics::describe_gauge!("ton_liteserver_requests", "Number of concurrent requests");

        let id = Cow::from(id);
        let client = ConcurrencyMetric::new(client, id.clone());
        let (mtx, mrx) = tokio::sync::watch::channel(None);
        let mut mc_watcher = mtx.subscribe();

        let _self = Self {
            id,
            client,

            masterchain_info_rx: mrx,
            registry: Default::default()
        };

        tokio::spawn(_self.last_block_loop(mtx));
        let inner = _self.first_block_loop();
        tokio::spawn(async move {
            mc_watcher.changed().await.unwrap();

            inner.await;
        });

        _self
    }

    fn last_block_loop(&self, mtx: Sender<Option<BlocksMasterchainInfo>>) -> impl Future<Output = Infallible> {
        let id = self.id.clone();
        let client = self.client.clone();
        let registry = self.registry.clone();

        let discover = LastBlockDiscover::new(id, client, registry, mtx);

        discover.discover()
    }

    fn first_block_loop(&self) -> impl Future<Output = Infallible> {
        let id = self.id.clone();
        let client = self.client.clone();
        let registry = self.registry.clone();

        let discover = FirstBlockDiscover::new(id, client, registry, self.masterchain_info_rx.clone());

        discover.discover()
    }

    fn edges_defined(&self) -> bool {
        let Some(master_shard_id) = self.masterchain_info_rx
            .borrow()
            .as_ref()
            .map(|info| (info.last.workchain, info.last.shard)) else { return false };

        self.registry.edges_defined(&master_shard_id)
    }
}

impl Service<Specialized<BlocksGetMasterchainInfo>> for CursorClient {
    type Response = BlocksMasterchainInfo;
    type Error = anyhow::Error;
    type Future = Pin<Box<dyn Future<Output = Result<Self::Response, Self::Error>> + Send>>;

    fn poll_ready(&mut self, cx: &mut Context<'_>) -> Poll<std::result::Result<(), Self::Error>> {
        if self.masterchain_info_rx.borrow().is_some() && self.edges_defined() {
            Poll::Ready(Ok(()))
        } else {
            cx.waker().wake_by_ref();

            Poll::Pending
        }
    }

    fn call(&mut self, _: Specialized<BlocksGetMasterchainInfo>) -> Self::Future {
        let response = self.masterchain_info_rx.borrow().as_ref().unwrap().clone();

        return ready(Ok(response)).boxed()
    }
}

impl<R : Callable<InnerClient>> Service<R> for CursorClient {
    type Response = R::Response;
    type Error = anyhow::Error;
    type Future = Pin<Box<dyn Future<Output = Result<Self::Response, Self::Error>> + Send>>;

    fn poll_ready(&mut self, cx: &mut Context<'_>) -> Poll<std::result::Result<(), Self::Error>> {
        if self.edges_defined() {
            return Service::<BlocksGetMasterchainInfo>::poll_ready(&mut self.client, cx);
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

async fn check_block_available(client: &mut InnerClient, block_id: TonBlockId) -> Result<(BlocksHeader, Vec<BlocksHeader>)> {
    let block_id = client.oneshot(BlocksLookupBlock::seqno(block_id)).await?;
    let shards = client.oneshot(BlocksGetShards::new(block_id.clone())).await?;

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
async fn find_first_blocks(client: &mut InnerClient, start: &TonBlockIdExt, lhs: Option<i32>, cur: Option<i32>) -> Result<(BlocksHeader, Vec<BlocksHeader>)> {
    let length = start.seqno;
    let mut rhs = length;
    let mut lhs = lhs.unwrap_or(1);
    let mut cur = cur.unwrap_or(start.seqno - 200000);

    let workchain = start.workchain;
    let shard = start.shard;

    let mut block = check_block_available(client, TonBlockId::new(workchain, shard, cur)).await;
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

        block = check_block_available(client, TonBlockId::new(workchain, shard, cur)).await;
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

    tracing::trace!(hops = hops, seqno = master.id.seqno, "first seqno");

    Ok((master, work))
}

async fn wait_for_block_header(block_id: TonBlockIdExt, client: InnerClient) -> Result<BlocksHeader> {
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
    rx: Receiver<Option<BlocksMasterchainInfo>>,
    current: Option<BlocksHeader>,
}

impl FirstBlockDiscover {
    fn new(id: Cow<'static, str>, client: InnerClient, registry: Arc<Registry>, rx: Receiver<Option<BlocksMasterchainInfo>>) -> Self {
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

    async fn next(&mut self, start: TonBlockIdExt) -> Result<Option<BlocksHeader>> {
        if let Some(ref mfb) = self.current {
            if let Err(e) = (&mut self.client).oneshot(BlocksGetShards::new(mfb.id.clone())).await {
                tracing::trace!(seqno = mfb.id.seqno, e = ?e, "first block not available anymore");
            } else {
                tracing::trace!("first block still available");

                return Ok(None);
            }
        }

        let lhs = self.current.as_ref().map(|n| n.id.seqno + 1);
        let cur = self.current.as_ref().map(|n| n.id.seqno + 32);
        let (mfb, wfb) = find_first_blocks(&mut self.client, &start, lhs, cur).await?;

        metrics::counter!("ton_liteserver_first_seqno", "liteserver_id" => self.id.clone()).absolute(mfb.id.seqno as u64);

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
    current: Option<BlocksMasterchainInfo>,
    mtx: Sender<Option<BlocksMasterchainInfo>>,
    last_block_tx: UnboundedSender<TonBlockIdExt>
}

impl LastBlockDiscover {
    fn new(id: Cow<'static, str>, client: InnerClient, registry: Arc<Registry>, mtx: Sender<Option<BlocksMasterchainInfo>>) -> Self {
        let (last_block_tx, mut rx) = tokio::sync::mpsc::unbounded_channel::<TonBlockIdExt>();

        // TODO[akostylev0] find last available block
        tokio::spawn({
            let client = client.clone();
            let registry = registry.clone();

            async move {
                let mut channels: HashMap<ShardId, UnboundedSender<TonBlockIdExt>> = Default::default();
                while let Some(block_id) = rx.recv().await {
                    let retry_strategy = FibonacciBackoff::from_millis(32).map(jitter).take(8);
                    match Retry::spawn(retry_strategy, || { client.clone().oneshot(BlocksGetShards::new(block_id.clone())) }).await {
                        Ok(shards) => {
                            let actual_shards: HashSet<ShardId> = HashSet::from_iter(
                                shards.shards.iter().map(|s| s.into())
                            );

                            for shard in shards.shards {
                                registry.upsert_right_end(&shard);

                                let shard_id: ShardId = (&shard).into();
                                let tx = if let Some(tx) = channels.get_mut(&shard_id) { tx } else {
                                    let (tx, mut rx) = tokio::sync::mpsc::unbounded_channel::<TonBlockIdExt>();

                                    tracing::info!(shard_id = ?shard_id, "spawn new channel for shard");
                                    tokio::spawn({
                                        let client = client.clone();
                                        let registry = registry.clone();

                                        async move {
                                            let registry = registry.clone();
                                            while let Some(block_id) = rx.recv().await {
                                                let retry_strategy = FibonacciBackoff::from_millis(32).map(jitter).take(16);
                                                match Retry::spawn(retry_strategy, || { client.clone().oneshot(BlocksGetBlockHeader::new(block_id.clone())) }).await {
                                                    Ok(header) => registry.upsert_right(&header),
                                                    Err(e) => {
                                                        tracing::warn!(error = ?e, "failed to get shard header");
                                                    }
                                                }
                                            }
                                        }
                                    });

                                    channels.insert(shard_id, tx);
                                    channels.get_mut(&shard_id).unwrap()
                                };

                                let _ = tx.send(shard);
                            }

                            channels.retain(|s, _| actual_shards.contains(s));
                        }
                        Err(error) => { tracing::warn!(error =?error, "get shards failed"); }
                    }
                }
            }
        });

        Self { id, client, registry, current: None, mtx, last_block_tx }
    }

    async fn discover(mut self) -> Never {
        let mut timer = interval(Duration::from_secs(1));
        timer.set_missed_tick_behavior(MissedTickBehavior::Skip);

        loop {
            timer.tick().await;

            metrics::gauge!("ton_liteserver_requests", "liteserver_id" => self.id.clone()).set(self.client.load() as f64);

            match self.next().await {
                Ok(Some(info)) => { self.current.replace(info); },
                Ok(None) => {}
                Err(_) => {}
            }
        }
    }

    async fn next(&mut self) -> Result<Option<BlocksMasterchainInfo>> {
        let mut info = (&mut self.client).oneshot(BlocksGetMasterchainInfo::new()).await?;
        metrics::counter!("ton_liteserver_last_seqno", "liteserver_id" => self.id.clone()).absolute(info.last.seqno as u64);
        if self.current.as_ref().is_some_and(|c| c == &info) {
            return Ok(None);
        }

        let last_block = (&mut self.client).oneshot(Sync::default()).await?;
        metrics::counter!("ton_liteserver_synced_seqno", "liteserver_id" => self.id.clone()).absolute(last_block.seqno as u64);
        self.registry.upsert_right_end(&last_block);

        while let Some(seqno) = self.registry.right_next((&last_block).into()) {
            let block_id = (&mut self.client).oneshot(BlocksLookupBlock::seqno(TonBlockId {
                workchain: last_block.workchain,
                shard: last_block.shard,
                seqno
            })).await?;

            let header = wait_for_block_header(block_id, self.client.clone()).await?;

            self.last_block_tx.send(last_block.clone())?;
            self.registry.upsert_right(&header);

            info.last = header.id;
            let _ = self.mtx.send(Some(info.clone()));
        }

        Ok(Some(info))
    }
}
