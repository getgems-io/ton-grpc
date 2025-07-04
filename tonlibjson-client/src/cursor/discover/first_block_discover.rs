use crate::block::{
    BlocksGetBlockHeader, BlocksGetShards, BlocksHeader, BlocksLookupBlock, BlocksMasterchainInfo,
    TonBlockId, TonBlockIdExt,
};
use crate::cursor::client::InnerClient;
use crate::cursor::registry::Registry;
use futures::never::Never;
use futures::try_join;
use std::borrow::Cow;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::watch::Receiver;
use tokio::time::{interval, MissedTickBehavior};
use tower::ServiceExt;
use tracing::instrument;

pub struct FirstBlockDiscover {
    id: Cow<'static, str>,
    client: InnerClient,
    registry: Arc<Registry>,
    rx: Receiver<Option<BlocksMasterchainInfo>>,
    current: Option<BlocksHeader>,
}

impl FirstBlockDiscover {
    pub fn new(
        id: Cow<'static, str>,
        client: InnerClient,
        registry: Arc<Registry>,
        rx: Receiver<Option<BlocksMasterchainInfo>>,
    ) -> Self {
        Self {
            id,
            client,
            registry,
            rx,
            current: None,
        }
    }

    pub async fn discover(mut self) -> Never {
        let mut timer = interval(Duration::from_secs(30));
        timer.set_missed_tick_behavior(MissedTickBehavior::Skip);

        loop {
            timer.tick().await;

            let Some(start) = self.rx.borrow().as_ref().map(|m| m.last.clone()) else {
                continue;
            };

            if let Ok(Some(mfb)) = self.next(start).await {
                self.current.replace(mfb);
            }
        }
    }

    async fn next(&mut self, start: TonBlockIdExt) -> anyhow::Result<Option<BlocksHeader>> {
        if let Some(ref mfb) = self.current {
            if let Err(e) = (&mut self.client)
                .oneshot(BlocksGetShards::new(mfb.id.clone()))
                .await
            {
                tracing::trace!(seqno = mfb.id.seqno, e = ?e, "first block not available anymore");
            } else {
                tracing::trace!("first block still available");

                return Ok(None);
            }
        }

        let lhs = self.current.as_ref().map(|n| n.id.seqno + 1);
        let cur = self.current.as_ref().map(|n| n.id.seqno + 32);
        let (mfb, wfb) = find_first_blocks(&mut self.client, &start, lhs, cur).await?;

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
async fn find_first_blocks(
    client: &mut InnerClient,
    start: &TonBlockIdExt,
    lhs: Option<i32>,
    cur: Option<i32>,
) -> anyhow::Result<(BlocksHeader, Vec<BlocksHeader>)> {
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
        if cur == 0 {
            break;
        }

        block = check_block_available(client, TonBlockId::new(workchain, shard, cur)).await;
        if block.is_ok() {
            success = Some(block.as_ref().unwrap().clone());
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

async fn check_block_available(
    client: &mut InnerClient,
    block_id: TonBlockId,
) -> anyhow::Result<(BlocksHeader, Vec<BlocksHeader>)> {
    let block_id = client.oneshot(BlocksLookupBlock::seqno(block_id)).await?;
    let shards = client
        .oneshot(BlocksGetShards::new(block_id.clone()))
        .await?;

    let clone = client.clone();
    let requests = shards
        .shards
        .into_iter()
        .map(BlocksGetBlockHeader::new)
        .map(|r| clone.clone().oneshot(r));

    try_join!(
        client.oneshot(BlocksGetBlockHeader::new(block_id)),
        futures::future::try_join_all(requests)
    )
}
