use crate::route::discover::shard_header::ShardHeaderActorHandle;
use crate::route::registry::Registry;
use crate::{RequestHandler, ShardId, shard_id_of};
use std::collections::{HashMap, HashSet};
use std::sync::Arc;
use tokio::sync::mpsc::{UnboundedReceiver, UnboundedSender};
use tokio_retry::Retry;
use tokio_retry::strategy::{FibonacciBackoff, jitter};
use ton_tower::actor::{AbortOnDropHandle, Actor};
use ton_tower::request::{GetBlockHeader, GetShards};
use ton_tower::response::BlockIdExt;
use tower::ServiceExt;

pub struct BlockShardsActorHandle {
    tx: UnboundedSender<BlockIdExt>,
    _handle: AbortOnDropHandle<()>,
}

impl BlockShardsActorHandle {
    pub fn new<S>(registry: Arc<Registry>, client: S) -> Self
    where
        S: RequestHandler<GetShards> + RequestHandler<GetBlockHeader> + Clone + Sync + 'static,
    {
        let (tx, rx) = tokio::sync::mpsc::unbounded_channel::<BlockIdExt>();
        let handle = BlockShardsActor::new(rx, registry, client).spawn_cancellable();

        Self {
            tx,
            _handle: handle,
        }
    }

    pub fn send(&self, block_id: BlockIdExt) -> anyhow::Result<()> {
        self.tx.send(block_id).map_err(Into::into)
    }
}

struct BlockShardsActor<S> {
    rx: UnboundedReceiver<BlockIdExt>,
    registry: Arc<Registry>,
    client: S,
}

impl<S> BlockShardsActor<S> {
    fn new(rx: UnboundedReceiver<BlockIdExt>, registry: Arc<Registry>, client: S) -> Self {
        BlockShardsActor {
            rx,
            registry,
            client,
        }
    }
}

impl<S> Actor for BlockShardsActor<S>
where
    S: RequestHandler<GetShards> + RequestHandler<GetBlockHeader> + Clone + Sync + 'static,
{
    type Output = ();

    async fn run(mut self) -> Self::Output {
        let mut channels: HashMap<ShardId, ShardHeaderActorHandle> = Default::default();
        while let Some(block_id) = self.rx.recv().await {
            let retry_strategy = FibonacciBackoff::from_millis(32).map(jitter).take(8);
            let shards = Retry::start(retry_strategy, || {
                let client = self.client.clone();
                let block_id = block_id.clone();
                client.oneshot(GetShards { block_id })
            })
            .await;

            match shards {
                Ok(shards) => {
                    let actual_shards: HashSet<ShardId> =
                        HashSet::from_iter(shards.iter().map(shard_id_of));

                    for shard in shards {
                        self.registry.upsert_right_end(&shard);

                        let shard_id: ShardId = shard_id_of(&shard);
                        let _ = channels
                            .entry(shard_id)
                            .or_insert_with(|| {
                                tracing::info!(shard_id = ?shard_id, "spawn new channel for shard");
                                ShardHeaderActorHandle::new(
                                    self.registry.clone(),
                                    self.client.clone(),
                                )
                            })
                            .send(shard);
                    }

                    channels.retain(|s, _| actual_shards.contains(s));
                }
                Err(error) => {
                    tracing::warn!(error =?error, "get shards failed");
                }
            }
        }
    }
}
