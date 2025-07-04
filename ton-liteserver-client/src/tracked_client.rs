use crate::tracker::masterchain_first_block_tracker::{
    MasterchainFirstBlockTracker, MasterchainFirstBlockTrackerActor,
};
use crate::tracker::masterchain_last_block_header_tracker::{
    MasterchainLastBlockHeaderTracker, MasterchainLastBlockHeaderTrackerActor,
};
use crate::tracker::masterchain_last_block_tracker::{
    MasterchainLastBlockTracker, MasterchainLastBlockTrackerActor,
};
use crate::tracker::workchains_first_blocks_tracker::{
    WorkchainsFirstBlocksTracker, WorkchainsFirstBlocksTrackerActor,
};
use crate::tracker::workchains_last_blocks_tracker::{
    WorkchainsLastBlocksTracker, WorkchainsLastBlocksTrackerActor,
};
use std::task::{Context, Poll};
use std::time::Duration;
use ton_client_util::actor::Actor;
use ton_client_util::router::route::BlockCriteria;
use ton_client_util::router::{BlockAvailability, Routed};
use ton_client_util::service::shared::SharedService;
use tower::load::peak_ewma::Cost;
use tower::load::{CompleteOnResponse, Load, PeakEwma};
use tower::Service;

#[derive(Debug, Clone)]
pub struct TrackedClient<S> {
    inner: SharedService<PeakEwma<S>>,
    masterchain_last_block_tracker: MasterchainLastBlockTracker,
    masterchain_last_block_header_tracker: MasterchainLastBlockHeaderTracker,
    masterchain_first_block_tracker: MasterchainFirstBlockTracker,
    workchains_last_blocks_tracker: WorkchainsLastBlocksTracker,
    workchains_first_blocks_tracker: WorkchainsFirstBlocksTracker,
}

impl<S> Load for TrackedClient<S> {
    type Metric = Cost;

    fn load(&self) -> Self::Metric {
        self.inner.load()
    }
}

impl<S> TrackedClient<S>
where
    S: Clone,
    MasterchainLastBlockTrackerActor<S>: Actor,
    MasterchainLastBlockHeaderTrackerActor<S>: Actor,
    MasterchainFirstBlockTrackerActor<S>: Actor,
    WorkchainsLastBlocksTrackerActor<S>: Actor,
    WorkchainsFirstBlocksTrackerActor<S>: Actor,
{
    pub fn new(inner: S) -> Self {
        let masterchain_last_block_tracker = MasterchainLastBlockTracker::new(inner.clone());
        let masterchain_last_block_header_tracker = MasterchainLastBlockHeaderTracker::new(
            inner.clone(),
            masterchain_last_block_tracker.clone(),
        );
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
            inner: SharedService::new(PeakEwma::new(
                inner,
                Duration::from_secs(1),
                Duration::from_millis(70).as_nanos() as f64,
                CompleteOnResponse::default(),
            )),
            masterchain_last_block_tracker,
            masterchain_last_block_header_tracker,
            masterchain_first_block_tracker,
            workchains_last_blocks_tracker,
            workchains_first_blocks_tracker,
        }
    }
}

impl<S> Routed for TrackedClient<S> {
    fn available(&self, chain: &i32, criteria: &BlockCriteria) -> BlockAvailability {
        let available = match chain {
            // masterchain
            -1 => self
                .masterchain_first_block_tracker
                .borrow()
                .as_ref()
                .zip(self.masterchain_last_block_header_tracker.borrow().as_ref())
                .is_some_and(|(lhs, rhs)| match criteria {
                    BlockCriteria::Seqno { seqno, .. } => {
                        lhs.info.seq_no as i32 <= *seqno && *seqno <= rhs.info.seq_no as i32
                    }
                    BlockCriteria::LogicalTime { lt, .. } => {
                        lhs.info.start_lt as i64 <= *lt && *lt <= rhs.info.end_lt as i64
                    }
                }),
            chain_id => match criteria {
                BlockCriteria::Seqno { shard, seqno } => self
                    .workchains_first_blocks_tracker
                    .get_first_block_id_for_shard(&(*chain_id, *shard))
                    .zip(
                        self.workchains_last_blocks_tracker
                            .get_shard(&(*chain_id, *shard)),
                    )
                    .is_some_and(|(lhs, rhs)| {
                        lhs.info.seq_no <= *seqno as u32 && *seqno as u32 <= rhs.seq_no
                    }),
                BlockCriteria::LogicalTime { address, lt } => self
                    .workchains_first_blocks_tracker
                    .find_min_lt_by_address(*chain_id, address)
                    .zip(
                        self.workchains_last_blocks_tracker
                            .find_max_lt_by_address(*chain_id, address),
                    )
                    .is_some_and(|(lhs, rhs)| lhs <= *lt as u64 && *lt as u64 <= rhs),
            },
        };

        if available {
            BlockAvailability::Available
        } else {
            BlockAvailability::NotPresent
        }
    }

    fn last_seqno(&self) -> Option<i32> {
        self.masterchain_last_block_tracker
            .borrow()
            .as_ref()
            .map(|info| info.last.seqno)
    }
}

impl<S, Request> Service<Request> for TrackedClient<S>
where
    S: Service<Request>,
{
    type Response = <SharedService<PeakEwma<S>> as Service<Request>>::Response;
    type Error = <SharedService<PeakEwma<S>> as Service<Request>>::Error;
    type Future = <SharedService<PeakEwma<S>> as Service<Request>>::Future;

    fn poll_ready(&mut self, cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        if self.masterchain_last_block_tracker.borrow().is_none() {
            cx.waker().wake_by_ref();

            return Poll::Pending;
        }

        self.inner.poll_ready(cx)
    }

    fn call(&mut self, req: Request) -> Self::Future {
        self.inner.call(req)
    }
}
