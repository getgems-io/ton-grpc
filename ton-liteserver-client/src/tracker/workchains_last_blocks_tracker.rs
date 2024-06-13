use crate::client::LiteServerClient;
use crate::tl::{LiteServerGetAllShardsInfo, TonNodeBlockIdExt};
use crate::tlb::shard_descr::ShardDescr;
use crate::tlb::shard_hashes::ShardHashes;
use crate::tracker::masterchain_last_block_tracker::MasterchainLastBlockTracker;
use tokio::select;
use tokio::sync::broadcast;
use tokio_util::sync::{CancellationToken, DropGuard};
use ton_client_utils::actor::cancellable_actor::CancellableActor;
use ton_client_utils::actor::Actor;
use toner::tlb::bits::de::unpack_bytes_fully;
use toner::tlb::ton::BoC;
use tower::ServiceExt;

struct WorkchainsLastBlocksActor {
    client: LiteServerClient,
    masterchain_last_block_tracker: MasterchainLastBlockTracker,
    sender: broadcast::Sender<TonNodeBlockIdExt>,
}

impl WorkchainsLastBlocksActor {
    pub fn new(
        client: LiteServerClient,
        masterchain_last_block_tracker: MasterchainLastBlockTracker,
        sender: broadcast::Sender<TonNodeBlockIdExt>,
    ) -> Self {
        Self {
            client,
            masterchain_last_block_tracker,
            sender,
        }
    }
}

pub struct WorkchainsLastBlocksTracker {
    receiver: broadcast::Receiver<TonNodeBlockIdExt>,
    _cancellation_token: DropGuard,
}

impl WorkchainsLastBlocksTracker {
    pub fn new(
        client: LiteServerClient,
        masterchain_last_block_tracker: MasterchainLastBlockTracker,
    ) -> Self {
        let cancellation_token = CancellationToken::new();

        let (sender, receiver) = broadcast::channel(64);
        CancellableActor::new(
            WorkchainsLastBlocksActor::new(client, masterchain_last_block_tracker, sender),
            cancellation_token.clone(),
        )
        .spawn();

        Self {
            receiver,
            _cancellation_token: cancellation_token.drop_guard(),
        }
    }

    pub fn receiver(&self) -> broadcast::Receiver<TonNodeBlockIdExt> {
        self.receiver.resubscribe()
    }
}

impl Actor for WorkchainsLastBlocksActor {
    type Output = ();

    async fn run(self) -> <Self as Actor>::Output {
        let mut client = self.client;
        let mut receiver = self.masterchain_last_block_tracker.receiver();
        loop {
            select! {
                Ok(_) = receiver.changed() => {
                    let info = receiver.borrow().clone().unwrap();

                    let shards_description = (&mut client)
                        .oneshot(LiteServerGetAllShardsInfo::new(info.last.clone()))
                        .await
                        .unwrap();

                    let boc: BoC = unpack_bytes_fully(&shards_description.data).unwrap();
                    let root = boc.single_root().unwrap();
                    let shard_hashes: ShardHashes = root.parse_fully().unwrap();

                    // TODO[akostylev0]: verify proofs
                    shard_hashes
                        .iter()
                        .map(|(chain_id, shards)| {
                            shards
                                .into_iter()
                                .map(move |shard: &ShardDescr| TonNodeBlockIdExt {
                                    workchain: *chain_id as i32,
                                    shard: shard.next_validator_shard as i64,
                                    seqno: shard.seq_no as i32,
                                    root_hash: shard.root_hash,
                                    file_hash: shard.file_hash,
                                })
                        })
                        .flatten()
                        .for_each(|shard_id| {
                            let _ = self.sender.send(shard_id).expect("expec to send shard_id");
                        });
                }
            }
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
