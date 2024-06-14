use crate::client::LiteServerClient;
use crate::tl::{LiteServerGetBlockHeader, LiteServerMasterchainInfo};
use crate::tracker::masterchain_last_block_tracker::MasterchainLastBlockTracker;
use std::future::Future;
use tokio::sync::watch;
use ton_client_utils::actor::Actor;
use tower::ServiceExt;

struct MasterchainLastBlockHeaderTrackerActor {
    client: LiteServerClient,
    masterchain_info_tracker: MasterchainLastBlockTracker,
}

impl Actor for MasterchainLastBlockHeaderTrackerActor {
    type Output = ();

    async fn run(mut self) -> <Self as Actor>::Output {
        let mut receiver = self.masterchain_info_tracker.receiver();

        while receiver.changed().await.is_ok() {
            let last_block_id = receiver
                .borrow()
                .as_ref()
                .expect("expect to get masterchain info")
                .last
                .clone();

            let header = (&mut self.client)
                .oneshot(LiteServerGetBlockHeader::new(last_block_id))
                .await
                .unwrap();

            unimplemented!()
        }
    }
}
