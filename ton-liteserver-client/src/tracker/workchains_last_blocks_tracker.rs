use futures::FutureExt;
use tokio::select;
use tokio::sync::{broadcast, watch};
use tokio::sync::watch::error::RecvError;
use tokio_util::sync::{CancellationToken, DropGuard};
use toner::tlb::bits::bitvec::order::Msb0;
use toner::tlb::bits::bitvec::vec::BitVec;
use toner::tlb::bits::de::{unpack_bytes_fully, unpack_fully};
use toner::tlb::ton::BoC;
use tower::ServiceExt;
use crate::client::LiteServerClient;
use crate::tl::{LiteServerGetAllShardsInfo, LiteServerGetBlockHeader, TonNodeBlockIdExt};
use crate::tlb::shard_descr::ShardDescr;
use crate::tlb::shard_hashes::ShardHashes;
use crate::tracker::masterchain_last_block_tracker::MasterchainLastBlockTracker;

struct WorkchainsLastBlocksActor {
    client: LiteServerClient,
    masterchain_last_block_tracker: MasterchainLastBlockTracker,
    cancellation_token: CancellationToken,
    sender: broadcast::Sender<TonNodeBlockIdExt>,
}

pub struct WorkchainsLastBlocksTracker {
    receiver: broadcast::Receiver<TonNodeBlockIdExt>,
    _cancellation_token: DropGuard,
}

impl WorkchainsLastBlocksTracker {
    pub fn new(client: LiteServerClient, masterchain_last_block_tracker: MasterchainLastBlockTracker) -> Self {
        let cancellation_token = CancellationToken::new();

        let (sender, receiver) = broadcast::channel(64);
        WorkchainsLastBlocksActor { client, masterchain_last_block_tracker, cancellation_token: cancellation_token.clone(), sender }.run();

        Self { receiver, _cancellation_token: cancellation_token.drop_guard() }
    }

    pub fn receiver(&self) -> broadcast::Receiver<TonNodeBlockIdExt> {
        self.receiver.resubscribe()
    }
}

impl WorkchainsLastBlocksActor {
    pub fn run(mut self) {
        tokio::spawn(async move {
            let mut client = self.client;
            let mut receiver = self.masterchain_last_block_tracker.receiver();
            loop {
                select! {
                    _ = self.cancellation_token.cancelled() => {
                        tracing::error!("MasterchainLastBlockTrackerActor cancelled");
                        break;
                    },
                    Ok(_) = receiver.changed() => {
                        let info = receiver.borrow().clone().unwrap();

                        let shards_description = (&mut client)
                            .oneshot(LiteServerGetAllShardsInfo::new(info.last.clone()))
                            .await
                            .unwrap();

                        let mut boc: BoC = unpack_bytes_fully(&shards_description.data).unwrap();
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
        });
    }
}


#[cfg(test)]
mod test {
    use tracing_test::traced_test;
    use crate::client::tests::provided_client;
    use crate::tracker::masterchain_first_block_tracker::MasterchainFirstBlockTracker;
    use crate::tracker::masterchain_last_block_tracker::MasterchainLastBlockTracker;
    use super::WorkchainsLastBlocksTracker;

    #[ignore]
    #[tokio::test]
    #[traced_test]
    async fn workchain_last_block_tracker() {
        let client = provided_client().await.unwrap();
        let last_tracker = MasterchainLastBlockTracker::new(client.clone());
        let mut workchain_tracker = WorkchainsLastBlocksTracker::new(client, last_tracker);

        let mut receiver = workchain_tracker.receiver();

        while let Ok(v) = receiver.recv().await {
            println!("{:x}", v.shard as u64);
        }
    }
}
