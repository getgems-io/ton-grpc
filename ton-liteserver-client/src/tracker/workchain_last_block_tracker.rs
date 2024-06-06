use tokio::select;
use tokio_util::sync::CancellationToken;
use tower::ServiceExt;
use crate::client::LiteServerClient;
use crate::tl::{LiteServerGetAllShardsInfo, TonNodeBlockIdExt};
use crate::tracker::masterchain_last_block_tracker::MasterchainLastBlockTracker;

struct WorkchainLastBlockActor {
    client: LiteServerClient,
    masterchain_last_block_tracker: MasterchainLastBlockTracker,
    cancellation_token: CancellationToken,
}

impl WorkchainLastBlockActor {
    pub fn run(mut self) {
        tokio::spawn(async move {
            let mut receiver = self.masterchain_last_block_tracker.receiver();
            let last_block_id = receiver.borrow().clone().unwrap().last;

            let shards_description = (&mut self.client)
                .oneshot(LiteServerGetAllShardsInfo::new(last_block_id))
                .await;
        });
    }
}

async fn resolve_shards_for_block(mut client: LiteServerClient, block_id: TonNodeBlockIdExt) {
    let shards_description = (&mut client)
        .oneshot(LiteServerGetAllShardsInfo::new(block_id))
        .await;
}
