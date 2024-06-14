use crate::client::LiteServerClient;
use crate::tracker::masterchain_first_block_tracker::MasterchainFirstBlockTracker;
use crate::tracker::masterchain_last_block_tracker::MasterchainLastBlockTracker;
use crate::tracker::workchains_first_blocks_tracker::WorkchainsFirstBlocksTracker;
use crate::tracker::workchains_last_blocks_tracker::WorkchainsLastBlocksTracker;
use ton_client_utils::router::{BlockCriteria, Routed};

pub struct TrackedClient {
    inner: LiteServerClient,
    masterchain_last_block_tracker: MasterchainLastBlockTracker,
    masterchain_first_block_tracker: MasterchainFirstBlockTracker,
    workchains_last_blocks_tracker: WorkchainsLastBlocksTracker,
    workchains_first_blocks_tracker: WorkchainsFirstBlocksTracker,
}

impl TrackedClient {
    pub fn new(inner: LiteServerClient) -> Self {
        let masterchain_last_block_tracker = MasterchainLastBlockTracker::new(inner.clone());
        let masterchain_first_block_tracker = MasterchainFirstBlockTracker::new(
            inner.clone(),
            masterchain_last_block_tracker.clone(),
        );
        let workchains_last_blocks_tracker =
            WorkchainsLastBlocksTracker::new(inner.clone(), masterchain_last_block_tracker.clone());
        let workchains_first_blocks_tracker = WorkchainsFirstBlocksTracker::new(
            inner.clone(),
            workchains_last_blocks_tracker.clone(),
        );

        Self {
            inner,
            masterchain_last_block_tracker,
            masterchain_first_block_tracker,
            workchains_last_blocks_tracker,
            workchains_first_blocks_tracker
        }
    }
}

impl Routed for TrackedClient {
    fn contains(&self, chain: &i32, criteria: &BlockCriteria) -> bool {
        match chain {
            // masterchain
            -1 => match criteria {
                BlockCriteria::Seqno { seqno, .. } => self
                    .masterchain_first_block_tracker
                    .borrow()
                    .as_ref()
                    .zip(self.masterchain_last_block_tracker.borrow().as_ref())
                    .is_some_and(|(header, info)| {
                        header.id.seqno <= *seqno && *seqno <= info.last.seqno
                    }),
                BlockCriteria::LogicalTime(lt) => {
                    unimplemented!()
                }
            },
            chain_id => match criteria {
                BlockCriteria::Seqno { shard, seqno } => {
                    let shard_id = (*chain_id, *shard);
                    self
                        .workchains_first_blocks_tracker
                        .get_first_block_id_for_shard(&shard_id)
                        .zip(self.workchains_last_blocks_tracker.get_shard(&shard_id))
                        .is_some_and(|(lhs, rhs)| {
                            lhs.seqno <= *seqno && *seqno <= rhs.seq_no as i32
                        })
                }
                BlockCriteria::LogicalTime(_) => {
                    // TODO[akostylev0] add account ad to criteria and apply shard_id as prefix

                    unimplemented!()
                }
            },
        }
    }

    fn contains_not_available(&self, chain: &i32, criteria: &BlockCriteria) -> bool {
        self.contains(chain, criteria)
    }

    fn last_seqno(&self) -> Option<i32> {
        self.masterchain_last_block_tracker
            .borrow()
            .as_ref()
            .map(|info| info.last.seqno)
    }
}
