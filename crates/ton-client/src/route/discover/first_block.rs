use crate::RequestHandler;
use crate::algo::binary_search::{BinarySearch, BlockAvailability};
use crate::route::discover::last_block::LastBlockDiscoverActorHandle;
use crate::route::registry::Registry;
use futures::never::Never;
use std::borrow::BorrowMut;
use std::sync::Arc;
use std::time::Duration;
use tokio::time::{MissedTickBehavior, interval};
use ton_tower::actor::{AbortOnDropHandle, Actor};
use ton_tower::request::{GetBlockHeader, GetMasterchainInfo, GetShards, LookUpBlockBySeqno};
use ton_tower::response::{BlockHeader, BlockIdExt};
use tower::ServiceExt;

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
        S: RequestHandler<GetMasterchainInfo>
            + RequestHandler<GetShards>
            + RequestHandler<LookUpBlockBySeqno>
            + RequestHandler<GetBlockHeader>
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
    S: RequestHandler<GetMasterchainInfo>
        + RequestHandler<GetShards>
        + RequestHandler<LookUpBlockBySeqno>
        + RequestHandler<GetBlockHeader>
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
    S: RequestHandler<GetMasterchainInfo>
        + RequestHandler<GetShards>
        + RequestHandler<LookUpBlockBySeqno>
        + RequestHandler<GetBlockHeader>
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

        let lhs = self.current.as_ref().map(|n| n.id.seqno);
        let cur = self.current.as_ref().map(|n| n.id.seqno + 32);
        let headers = BlockAvailability::new(&mut self.client)
            .starting_at(cur.unwrap_or(start.seqno - 200000))
            .with_tolerance(4)
            .from(lhs)
            .to(Some(start.into()))
            .find()
            .await?;

        for header in &headers {
            self.registry.upsert_left(header);
        }

        let Some(mfb) = headers.into_iter().next() else {
            return Ok(None);
        };

        metrics::counter!("ton_liteserver_first_seqno", "liteserver_id" => self.id.clone())
            .absolute(mfb.id.seqno as u64);

        Ok(Some(mfb))
    }
}
