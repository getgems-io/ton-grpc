use crate::client::LiteServerClient;
use crate::tl::{LiteServerGetAllShardsInfo, TonNodeBlockIdExt};
use crate::tlb::shard_descr::ShardDescr;
use crate::tlb::shard_hashes::ShardHashes;
use crate::tracker::masterchain_last_block_tracker::MasterchainLastBlockTracker;
use crate::tracker::ShardId;
use dashmap::DashMap;
use std::sync::Arc;
use tokio::sync::broadcast;
use tokio_util::sync::{CancellationToken, DropGuard};
use ton_client_util::actor::cancellable_actor::CancellableActor;
use ton_client_util::actor::Actor;
use toner::tlb::bits::de::unpack_bytes_fully;
use toner::ton::boc::BoC;
use tower::ServiceExt;
use ton_client_util::router::shards::Prefix;

struct WorkchainsLastBlocksActor {
    client: LiteServerClient,
    masterchain_last_block_tracker: MasterchainLastBlockTracker,
    sender: broadcast::Sender<TonNodeBlockIdExt>,
    state: Arc<DashMap<ShardId, ShardDescr>>,
}

impl WorkchainsLastBlocksActor {
    pub fn new(
        client: LiteServerClient,
        masterchain_last_block_tracker: MasterchainLastBlockTracker,
        sender: broadcast::Sender<TonNodeBlockIdExt>,
        state: Arc<DashMap<ShardId, ShardDescr>>,
    ) -> Self {
        Self {
            client,
            masterchain_last_block_tracker,
            sender,
            state,
        }
    }
}

pub struct WorkchainsLastBlocksTracker {
    receiver: broadcast::Receiver<TonNodeBlockIdExt>,
    state: Arc<DashMap<ShardId, ShardDescr>>,
    _cancellation_token: Arc<DropGuard>,
}

impl Clone for WorkchainsLastBlocksTracker {
    fn clone(&self) -> Self {
        Self {
            receiver: self.receiver.resubscribe(),
            state: Arc::clone(&self.state),
            _cancellation_token: Arc::clone(&self._cancellation_token),
        }
    }
}

impl WorkchainsLastBlocksTracker {
    pub fn new(
        client: LiteServerClient,
        masterchain_last_block_tracker: MasterchainLastBlockTracker,
    ) -> Self {
        let state = Arc::new(DashMap::default());
        let cancellation_token = CancellationToken::new();

        let (sender, receiver) = broadcast::channel(64);
        CancellableActor::new(
            WorkchainsLastBlocksActor::new(
                client,
                masterchain_last_block_tracker,
                sender,
                Arc::clone(&state),
            ),
            cancellation_token.clone(),
        )
        .spawn();

        Self {
            receiver,
            state,
            _cancellation_token: Arc::new(cancellation_token.drop_guard()),
        }
    }

    pub fn get_shard(&self, shard_id: &ShardId) -> Option<ShardDescr> {
        self.state.view(shard_id, |_, shard| shard.clone())
    }

    pub fn receiver(&self) -> broadcast::Receiver<TonNodeBlockIdExt> {
        self.receiver.resubscribe()
    }

    pub fn find_max_lt_by_address(&self, chain_id: i32, address: &[u8; 32]) -> Option<u64> {
        self.state
            .iter()
            .filter_map(|kv| {
                let key = kv.key();

                (key.0 == chain_id && Prefix::from_shard_id(key.1 as u64).matches(address)).then(|| kv.value().end_lt)
            })
            .max()
    }
}

impl Actor for WorkchainsLastBlocksActor {
    type Output = ();

    async fn run(mut self) -> <Self as Actor>::Output {
        let mut receiver = self.masterchain_last_block_tracker.receiver();

        while receiver.changed().await.is_ok() {
            let last_block_id = receiver
                .borrow()
                .as_ref()
                .expect("expect to get masterchain info")
                .last
                .clone();

            let shards_description = (&mut self.client)
                .oneshot(LiteServerGetAllShardsInfo::new(last_block_id))
                .await
                .unwrap();

            let boc: BoC = unpack_bytes_fully(&shards_description.data).unwrap();
            let root = boc.single_root().unwrap();
            let shard_hashes: ShardHashes = root.parse_fully().unwrap();

            // TODO[akostylev0]: verify proofs
            shard_hashes
                .iter()
                .flat_map(|(chain_id, shards)| shards.iter().map(move |shard| (chain_id, shard)))
                .for_each(|(chain_id, shard)| {
                    let shard_id = (*chain_id as i32, shard.next_validator_shard as i64);
                    self.state.insert(shard_id, shard.clone());

                    let _ = self
                        .sender
                        .send(TonNodeBlockIdExt {
                            workchain: *chain_id as i32,
                            shard: shard.next_validator_shard as i64,
                            seqno: shard.seq_no as i32,
                            root_hash: shard.root_hash,
                            file_hash: shard.file_hash,
                        })
                        .expect("expect to send shard_id");
                });
        }
    }
}

#[cfg(test)]
mod test {
    use super::WorkchainsLastBlocksTracker;
    use crate::client::tests::provided_client;
    use crate::tracker::masterchain_last_block_tracker::MasterchainLastBlockTracker;
    use tracing_test::traced_test;

    #[ignore]
    #[tokio::test]
    #[traced_test]
    async fn workchain_last_block_tracker() {
        let client = provided_client().await.unwrap();
        let last_tracker = MasterchainLastBlockTracker::new(client.clone());
        let workchain_tracker = WorkchainsLastBlocksTracker::new(client, last_tracker);

        let mut receiver = workchain_tracker.receiver();

        while let Ok(v) = receiver.recv().await {
            println!("{:x}", v.shard as u64);
        }
    }
}
