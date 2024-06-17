use crate::client::LiteServerClient;
use crate::tl::{LiteServerBlockHeader, TonNodeBlockIdExt};
use crate::tracker::find_first_block::find_first_block_header;
use crate::tracker::workchains_last_blocks_tracker::WorkchainsLastBlocksTracker;
use adnl_tcp::types::{Int, Long};
use futures::stream::FuturesUnordered;
use futures::StreamExt;
use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;
use dashmap::DashMap;
use tokio::select;
use tokio::sync::broadcast;
use tokio::time::Instant;
use tokio_util::sync::{CancellationToken, DropGuard};
use toner::tlb::bits::de::unpack_bytes_fully;
use toner::ton::boc::BoC;
use ton_client_util::actor::cancellable_actor::CancellableActor;
use ton_client_util::actor::Actor;
use crate::tlb::block_header::BlockHeader;
use crate::tracker::ShardId;

struct WorkchainsFirstBlocksTrackerActor {
    client: LiteServerClient,
    last_block_tracker: WorkchainsLastBlocksTracker,
    state: Arc<DashMap<ShardId, BlockHeader>>,
    sender: broadcast::Sender<BlockHeader>,
}

impl WorkchainsFirstBlocksTrackerActor {
    pub fn new(
        client: LiteServerClient,
        last_block_tracker: WorkchainsLastBlocksTracker,
        state: Arc<DashMap<ShardId, BlockHeader>>,
        sender: broadcast::Sender<BlockHeader>,
    ) -> Self {
        Self {
            client,
            last_block_tracker,
            state,
            sender,
        }
    }
}

impl Actor for WorkchainsFirstBlocksTrackerActor {
    type Output = ();

    async fn run(self) -> <Self as Actor>::Output {
        let mut futures = FuturesUnordered::new();
        let mut receiver = self.last_block_tracker.receiver();
        let mut timeouts: HashMap<(Int, Long), Instant> = HashMap::default();

        loop {
            select! {
                Ok(block_id) = receiver.recv() => {
                    let shard_id = (block_id.workchain, block_id.shard);

                    if timeouts
                        .get(&shard_id)
                        .filter(|time| time.elapsed() < Duration::from_secs(60))
                        .is_none()
                    {
                        timeouts.remove(&shard_id);
                        timeouts.insert(shard_id, Instant::now());

                        futures.push(find_first_block_header(self.client.clone(), block_id, None, None));
                    }
                },
                Some(result) = futures.next() => {
                    match result {
                        Ok(resolved) => {
                            let shard_id = (resolved.id.workchain, resolved.id.shard);

                            let boc: BoC = unpack_bytes_fully(&resolved.header_proof).unwrap();
                            let root = boc.single_root().unwrap();
                            let block_header: BlockHeader = root.parse_fully().unwrap();

                            self.state.insert(shard_id, block_header.clone());

                            let _ = self.sender.send(block_header).unwrap();
                        },
                        Err(e) => { tracing::error!("Error: {:?}", e); }
                    }
                }
            }
        }
    }
}

pub struct WorkchainsFirstBlocksTracker {
    receiver: broadcast::Receiver<BlockHeader>,
    state: Arc<DashMap<ShardId, BlockHeader>>,
    _cancellation_token: DropGuard,
}

impl WorkchainsFirstBlocksTracker {
    pub fn new(client: LiteServerClient, last_block_tracker: WorkchainsLastBlocksTracker) -> Self {
        let state = Arc::new(DashMap::default());
        let cancellation_token = CancellationToken::new();
        let (sender, receiver) = broadcast::channel(64);

        CancellableActor::new(
            WorkchainsFirstBlocksTrackerActor::new(client, last_block_tracker, Arc::clone(&state), sender),
            cancellation_token.clone(),
        )
        .spawn();

        Self {
            receiver,
            state,
            _cancellation_token: cancellation_token.drop_guard(),
        }
    }

    pub fn receiver(&self) -> broadcast::Receiver<BlockHeader> {
        self.receiver.resubscribe()
    }

    pub fn get_first_block_id_for_shard(&self, shard_id: &ShardId) -> Option<BlockHeader> {
        self.state.view(shard_id, |_, header| header.clone())
    }
}

#[cfg(test)]
mod test {
    use super::WorkchainsFirstBlocksTracker;
    use crate::client::tests::provided_client;
    use crate::tracker::masterchain_last_block_tracker::MasterchainLastBlockTracker;
    use crate::tracker::workchains_last_blocks_tracker::WorkchainsLastBlocksTracker;
    use tracing_test::traced_test;

    #[ignore]
    #[tokio::test]
    #[traced_test]
    async fn workchains_first_block_tracker() {
        let client = provided_client().await.unwrap();
        let masterchain_tracker = MasterchainLastBlockTracker::new(client.clone());
        let workchain_tracker =
            WorkchainsLastBlocksTracker::new(client.clone(), masterchain_tracker);
        let first_tracker = WorkchainsFirstBlocksTracker::new(client.clone(), workchain_tracker);

        let mut receiver = first_tracker.receiver();

        println!("wait");

        while let Ok(block) = receiver.recv().await {
            println!("{:?}", block)
        }
    }
}
