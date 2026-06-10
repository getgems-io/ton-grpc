use crate::RequestHandler;
use crate::actor::Actor;
use crate::route::discover::last_block::LastBlockDiscoverActorHandle;
use crate::route::registry::Registry;
use futures::never::Never;
use futures::try_join;
use std::borrow::BorrowMut;
use std::sync::Arc;
use std::time::Duration;
use tokio::time::{MissedTickBehavior, interval};
use tokio_util::task::AbortOnDropHandle;
use ton_tower::request::{GetBlockHeader, GetShards, LookUpBlockBySeqno};
use ton_tower::response::{BlockHeader, BlockIdExt};
use tower::ServiceExt;
use tracing::instrument;

#[derive(Clone)]
pub struct FirstBlockDiscoverActorHandle {
    _handle: Arc<AbortOnDropHandle<Never>>,
}

impl FirstBlockDiscoverActorHandle {
    pub fn new<S>(
        id: String,
        registry: Arc<Registry>,
        client: S,
        rx: LastBlockDiscoverActorHandle,
    ) -> Self
    where
        S: RequestHandler<GetShards>
            + RequestHandler<LookUpBlockBySeqno>
            + RequestHandler<GetBlockHeader>
            + Clone
            + Send
            + 'static,
    {
        let handle = FirstBlockDiscover::new(id, registry, client, rx).spawn_cancellable();

        Self {
            _handle: Arc::new(handle),
        }
    }
}

struct FirstBlockDiscover<S> {
    id: String,
    client: S,
    registry: Arc<Registry>,
    rx: LastBlockDiscoverActorHandle,
    current: Option<BlockHeader>,
}

impl<S> Actor for FirstBlockDiscover<S>
where
    S: RequestHandler<GetShards>
        + RequestHandler<LookUpBlockBySeqno>
        + RequestHandler<GetBlockHeader>
        + Clone
        + Send
        + 'static,
{
    type Output = Never;

    async fn run(mut self) -> Self::Output {
        self.rx
            .changed()
            .await
            .expect("failed to wait for last block");

        let mut timer = interval(Duration::from_secs(30));
        timer.set_missed_tick_behavior(MissedTickBehavior::Skip);

        loop {
            timer.tick().await;

            let Some(start) = self.rx.last_value().as_ref().map(|m| m.last.clone()) else {
                continue;
            };

            if let Ok(Some(mfb)) = self.next(start).await {
                self.current.replace(mfb);
            }
        }
    }
}

impl<S> FirstBlockDiscover<S>
where
    S: RequestHandler<GetShards>
        + RequestHandler<LookUpBlockBySeqno>
        + RequestHandler<GetBlockHeader>
        + Clone
        + Send
        + 'static,
{
    fn new(
        id: String,
        registry: Arc<Registry>,
        client: S,
        rx: LastBlockDiscoverActorHandle,
    ) -> Self {
        Self {
            id,
            client,
            registry,
            rx,
            current: None,
        }
    }

    async fn next(&mut self, start: BlockIdExt) -> anyhow::Result<Option<BlockHeader>> {
        if let Some(ref mfb) = self.current {
            let probe = self
                .client
                .borrow_mut()
                .oneshot(GetShards {
                    block_id: mfb.id.clone(),
                })
                .await;
            if let Err(e) = probe {
                tracing::trace!(seqno = mfb.id.seqno, e = ?e, "first block not available anymore");
            } else {
                tracing::trace!("first block still available");

                return Ok(None);
            }
        }

        let lhs = self.current.as_ref().map(|n| n.id.seqno + 1);
        let cur = self.current.as_ref().map(|n| n.id.seqno + 32);
        let (mfb, wfb) = find_first_blocks(self.client.borrow_mut(), &start, lhs, cur).await?;

        metrics::counter!("ton_liteserver_first_seqno", "liteserver_id" => self.id.clone())
            .absolute(mfb.id.seqno as u64);

        self.registry.upsert_left(&mfb);
        for header in &wfb {
            self.registry.upsert_left(header);
        }

        Ok(Some(mfb))
    }
}

#[instrument(skip_all, err, level = "trace")]
async fn find_first_blocks<S>(
    client: &mut S,
    start: &BlockIdExt,
    lhs: Option<i32>,
    cur: Option<i32>,
) -> anyhow::Result<(BlockHeader, Vec<BlockHeader>)>
where
    S: RequestHandler<GetShards>
        + RequestHandler<LookUpBlockBySeqno>
        + RequestHandler<GetBlockHeader>
        + Clone
        + Send
        + 'static,
{
    let length = start.seqno;
    let mut rhs = length;
    let mut lhs = lhs.unwrap_or(1);
    let mut cur = cur.unwrap_or(start.seqno - 200000);

    let workchain = start.workchain;
    let shard = start.shard;

    let mut block = check_block_available(client, workchain, shard, cur).await;
    let mut success = None;

    let mut hops = 0;

    while lhs < rhs {
        if block.is_err() {
            lhs = cur + 1;
        } else {
            rhs = cur;
        }

        cur = (lhs + rhs) / 2;
        if cur == 0 {
            break;
        }

        block = check_block_available(client, workchain, shard, cur).await;
        if let Ok(inner) = &block {
            success = Some(inner.clone());
        }

        hops += 1;
    }

    let delta = 4;
    let (master, work) = match block {
        Ok(b) => b,
        Err(e) => match success {
            Some(b) if b.0.id.seqno - cur <= delta => b,
            _ => return Err(e),
        },
    };

    tracing::trace!(hops = hops, seqno = master.id.seqno, "first seqno");

    Ok((master, work))
}

// TODO[akostylev0]: remove Clone
async fn check_block_available<S>(
    client: &mut S,
    workchain: i32,
    shard: i64,
    seqno: i32,
) -> anyhow::Result<(BlockHeader, Vec<BlockHeader>)>
where
    S: RequestHandler<GetShards>
        + RequestHandler<LookUpBlockBySeqno>
        + RequestHandler<GetBlockHeader>
        + Clone
        + Send
        + 'static,
{
    let block_id = client
        .oneshot(LookUpBlockBySeqno {
            chain: workchain,
            shard,
            seqno,
        })
        .await?;
    let shards = client
        .oneshot(GetShards {
            block_id: block_id.clone(),
        })
        .await?;

    let requests = shards.into_iter().map(|id| {
        let client = client.clone();
        client.oneshot(GetBlockHeader { id })
    });

    // TODO[akostylev0]: use `ServiceExt::call_all`
    try_join!(
        client.clone().oneshot(GetBlockHeader { id: block_id }),
        futures::future::try_join_all(requests)
    )
}
