use crate::RequestHandler;
use crate::actor::Actor;
use crate::route::discover::block_shards::BlockShardsActorHandle;
use crate::route::registry::Registry;
use crate::route::shard_id_of;
use anyhow::Result;
use futures::never::Never;
use std::borrow::BorrowMut;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::watch::{Receiver, Ref, Sender, error};
use tokio::time::{MissedTickBehavior, interval};
use tokio_retry::Retry;
use tokio_retry::strategy::{FibonacciBackoff, jitter};
use tokio_util::task::AbortOnDropHandle;
use ton_tower::request::{GetBlockHeader, GetMasterchainInfo, GetShards, LookUpBlockBySeqno, Sync};
use ton_tower::response::{BlockHeader, BlockIdExt, MasterchainInfo};
use tower::ServiceExt;
use tower::load::Load;

#[derive(Clone)]
pub struct LastBlockDiscoverActorHandle {
    mrx: Receiver<Option<MasterchainInfo>>,
    _handle: Arc<AbortOnDropHandle<Never>>,
}

impl LastBlockDiscoverActorHandle {
    pub fn new<S>(id: String, registry: Arc<Registry>, client: S) -> Self
    where
        S: RequestHandler<GetMasterchainInfo>
            + RequestHandler<Sync>
            + RequestHandler<LookUpBlockBySeqno>
            + RequestHandler<GetBlockHeader>
            + RequestHandler<GetShards>
            + Load
            + Clone
            + std::marker::Sync
            + 'static,
        S::Metric: Into<f64>,
    {
        let (mtx, mrx) = tokio::sync::watch::channel(None);

        let handle = LastBlockDiscoverActor::new(id, registry, client, mtx).spawn_cancellable();

        Self {
            mrx,
            _handle: Arc::new(handle),
        }
    }

    pub async fn changed(&mut self) -> Result<(), error::RecvError> {
        self.mrx.changed().await
    }

    pub fn last_value(&self) -> Ref<'_, Option<MasterchainInfo>> {
        self.mrx.borrow()
    }
}

struct LastBlockDiscoverActor<S> {
    id: String,
    client: S,
    registry: Arc<Registry>,
    current: Option<MasterchainInfo>,
    mtx: Sender<Option<MasterchainInfo>>,
    block_shards_actor_handle: BlockShardsActorHandle,
}

impl<S> Actor for LastBlockDiscoverActor<S>
where
    S: RequestHandler<GetMasterchainInfo>
        + RequestHandler<Sync>
        + RequestHandler<LookUpBlockBySeqno>
        + RequestHandler<GetBlockHeader>
        + RequestHandler<GetShards>
        + Load
        + Clone
        + std::marker::Sync
        + 'static,
    S::Metric: Into<f64>,
{
    type Output = Never;

    async fn run(mut self) -> Self::Output {
        let mut timer = interval(Duration::from_secs(1));
        timer.set_missed_tick_behavior(MissedTickBehavior::Skip);

        loop {
            timer.tick().await;

            metrics::gauge!("ton_liteserver_requests", "liteserver_id" => self.id.clone())
                .set(self.client.load().into());

            match self.next().await {
                Ok(Some(info)) => {
                    self.current.replace(info);
                }
                Ok(None) => {}
                Err(_) => {}
            }
        }
    }
}

impl<S> LastBlockDiscoverActor<S>
where
    S: RequestHandler<GetMasterchainInfo>
        + RequestHandler<Sync>
        + RequestHandler<LookUpBlockBySeqno>
        + RequestHandler<GetBlockHeader>
        + RequestHandler<GetShards>
        + Load
        + Clone
        + std::marker::Sync
        + 'static,
    S::Metric: Into<f64>,
{
    pub fn new(
        id: String,
        registry: Arc<Registry>,
        client: S,
        mtx: Sender<Option<MasterchainInfo>>,
    ) -> Self {
        let block_shards_actor_handle =
            BlockShardsActorHandle::new(registry.clone(), client.clone());

        Self {
            id,
            client,
            registry,
            current: None,
            mtx,
            block_shards_actor_handle,
        }
    }

    async fn next(&mut self) -> Result<Option<MasterchainInfo>> {
        let mut info: MasterchainInfo = self
            .client
            .borrow_mut()
            .oneshot(GetMasterchainInfo::default())
            .await?;
        metrics::counter!("ton_liteserver_last_seqno", "liteserver_id" => self.id.clone())
            .absolute(info.last.seqno as u64);
        if self.current.as_ref().is_some_and(|c| c == &info) {
            return Ok(None);
        }

        let last_block = self.client.borrow_mut().oneshot(Sync::default()).await?;
        metrics::counter!("ton_liteserver_synced_seqno", "liteserver_id" => self.id.clone())
            .absolute(last_block.seqno as u64);
        self.registry.upsert_right_end(&last_block);

        while let Some(seqno) = self.registry.right_next(shard_id_of(&last_block)) {
            let block_id = self
                .client
                .borrow_mut()
                .oneshot(LookUpBlockBySeqno {
                    chain: last_block.workchain,
                    shard: last_block.shard,
                    seqno,
                })
                .await?;

            let header = wait_for_block_header(self.client.borrow_mut(), block_id).await?;

            self.block_shards_actor_handle.send(last_block.clone())?;
            self.registry.upsert_right(&header);

            info.last = header.id.clone();
            let _ = self.mtx.send(Some(info.clone()));
        }

        Ok(Some(info))
    }
}

async fn wait_for_block_header<S>(client: &mut S, block_id: BlockIdExt) -> Result<BlockHeader>
where
    S: RequestHandler<GetBlockHeader> + Clone,
{
    let retry = FibonacciBackoff::from_millis(512)
        .max_delay(Duration::from_millis(4096))
        .map(jitter)
        .take(16);

    Retry::spawn(retry, || {
        let block_id = block_id.clone();
        let client = client.clone();

        client.oneshot(GetBlockHeader { id: block_id })
    })
    .await
}
