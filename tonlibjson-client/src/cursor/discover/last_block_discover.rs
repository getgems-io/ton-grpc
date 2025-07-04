use crate::block::{
    BlocksGetBlockHeader, BlocksGetMasterchainInfo, BlocksGetShards, BlocksHeader,
    BlocksLookupBlock, BlocksMasterchainInfo, Sync, TonBlockId, TonBlockIdExt,
};
use crate::cursor::client::InnerClient;
use crate::cursor::registry::Registry;
use crate::cursor::ShardId;
use anyhow::Result;
use futures::never::Never;
use std::borrow::Cow;
use std::collections::{HashMap, HashSet};
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::mpsc::UnboundedSender;
use tokio::sync::watch::Sender;
use tokio::time::{interval, MissedTickBehavior};
use tokio_retry::strategy::{jitter, FibonacciBackoff};
use tokio_retry::Retry;
use tower::load::Load;
use tower::ServiceExt;

pub struct LastBlockDiscover {
    id: Cow<'static, str>,
    client: InnerClient,
    registry: Arc<Registry>,
    current: Option<BlocksMasterchainInfo>,
    mtx: Sender<Option<BlocksMasterchainInfo>>,
    last_block_tx: UnboundedSender<TonBlockIdExt>,
}

impl LastBlockDiscover {
    pub fn new(
        id: Cow<'static, str>,
        client: InnerClient,
        registry: Arc<Registry>,
        mtx: Sender<Option<BlocksMasterchainInfo>>,
    ) -> Self {
        let (last_block_tx, mut rx) = tokio::sync::mpsc::unbounded_channel::<TonBlockIdExt>();

        // TODO[akostylev0] find last available block
        tokio::spawn({
            let client = client.clone();
            let registry = registry.clone();

            async move {
                let mut channels: HashMap<ShardId, UnboundedSender<TonBlockIdExt>> =
                    Default::default();
                while let Some(block_id) = rx.recv().await {
                    let retry_strategy = FibonacciBackoff::from_millis(32).map(jitter).take(8);
                    match Retry::spawn(retry_strategy, || {
                        client
                            .clone()
                            .oneshot(BlocksGetShards::new(block_id.clone()))
                    })
                    .await
                    {
                        Ok(shards) => {
                            let actual_shards: HashSet<ShardId> =
                                HashSet::from_iter(shards.shards.iter().map(|s| s.into()));

                            for shard in shards.shards {
                                registry.upsert_right_end(&shard);

                                let shard_id: ShardId = (&shard).into();
                                let tx = if let Some(tx) = channels.get_mut(&shard_id) {
                                    tx
                                } else {
                                    let (tx, mut rx) =
                                        tokio::sync::mpsc::unbounded_channel::<TonBlockIdExt>();

                                    tracing::info!(shard_id = ?shard_id, "spawn new channel for shard");
                                    tokio::spawn({
                                        let client = client.clone();
                                        let registry = registry.clone();

                                        async move {
                                            let registry = registry.clone();
                                            while let Some(block_id) = rx.recv().await {
                                                let retry_strategy =
                                                    FibonacciBackoff::from_millis(32)
                                                        .map(jitter)
                                                        .take(16);
                                                match Retry::spawn(retry_strategy, || {
                                                    client.clone().oneshot(
                                                        BlocksGetBlockHeader::new(block_id.clone()),
                                                    )
                                                })
                                                .await
                                                {
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
                        Err(error) => {
                            tracing::warn!(error =?error, "get shards failed");
                        }
                    }
                }
            }
        });

        Self {
            id,
            client,
            registry,
            current: None,
            mtx,
            last_block_tx,
        }
    }

    pub async fn discover(mut self) -> Never {
        let mut timer = interval(Duration::from_secs(1));
        timer.set_missed_tick_behavior(MissedTickBehavior::Skip);

        loop {
            timer.tick().await;

            metrics::gauge!("ton_liteserver_requests", "liteserver_id" => self.id.clone())
                .set(self.client.load() as f64);

            match self.next().await {
                Ok(Some(info)) => {
                    self.current.replace(info);
                }
                Ok(None) => {}
                Err(_) => {}
            }
        }
    }

    async fn next(&mut self) -> Result<Option<BlocksMasterchainInfo>> {
        let mut info = (&mut self.client)
            .oneshot(BlocksGetMasterchainInfo::new())
            .await?;
        metrics::counter!("ton_liteserver_last_seqno", "liteserver_id" => self.id.clone())
            .absolute(info.last.seqno as u64);
        if self.current.as_ref().is_some_and(|c| c == &info) {
            return Ok(None);
        }

        let last_block = (&mut self.client).oneshot(Sync::default()).await?;
        metrics::counter!("ton_liteserver_synced_seqno", "liteserver_id" => self.id.clone())
            .absolute(last_block.seqno as u64);
        self.registry.upsert_right_end(&last_block);

        while let Some(seqno) = self.registry.right_next((&last_block).into()) {
            let block_id = (&mut self.client)
                .oneshot(BlocksLookupBlock::seqno(TonBlockId {
                    workchain: last_block.workchain,
                    shard: last_block.shard,
                    seqno,
                }))
                .await?;

            let header = wait_for_block_header(block_id, self.client.clone()).await?;

            self.last_block_tx.send(last_block.clone())?;
            self.registry.upsert_right(&header);

            info.last = header.id;
            let _ = self.mtx.send(Some(info.clone()));
        }

        Ok(Some(info))
    }
}

async fn wait_for_block_header(
    block_id: TonBlockIdExt,
    client: InnerClient,
) -> anyhow::Result<BlocksHeader> {
    let retry = FibonacciBackoff::from_millis(512)
        .max_delay(Duration::from_millis(4096))
        .map(jitter)
        .take(16);

    Retry::spawn(retry, || {
        let block_id = block_id.clone();
        let client = client.clone();

        client.oneshot(BlocksGetBlockHeader::new(block_id))
    })
    .await
}
