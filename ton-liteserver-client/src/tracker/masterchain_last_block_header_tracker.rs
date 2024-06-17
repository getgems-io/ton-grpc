use crate::client::LiteServerClient;
use crate::tl::LiteServerGetBlockHeader;
use crate::tlb::block_header::BlockHeader;
use crate::tracker::masterchain_last_block_tracker::MasterchainLastBlockTracker;
use std::future::Future;
use tokio::sync::watch;
use tokio_util::sync::{CancellationToken, DropGuard};
use ton_client_utils::actor::cancellable_actor::CancellableActor;
use ton_client_utils::actor::Actor;
use toner::tlb::bits::de::unpack_bytes_fully;
use toner::ton::boc::BoC;
use tower::ServiceExt;

struct MasterchainLastBlockHeaderTrackerActor {
    client: LiteServerClient,
    masterchain_info_tracker: MasterchainLastBlockTracker,
    sender: watch::Sender<Option<BlockHeader>>,
}

impl MasterchainLastBlockHeaderTrackerActor {
    pub fn new(
        client: LiteServerClient,
        masterchain_info_tracker: MasterchainLastBlockTracker,
        sender: watch::Sender<Option<BlockHeader>>,
    ) -> Self {
        Self {
            client,
            masterchain_info_tracker,
            sender,
        }
    }
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

            let header_bytes = (&mut self.client)
                .oneshot(LiteServerGetBlockHeader::new(last_block_id))
                .await
                .unwrap()
                .header_proof;

            let boc: BoC = unpack_bytes_fully(&header_bytes).unwrap();
            let root = boc.single_root().unwrap();

            let header: BlockHeader = root.parse_fully().unwrap();

            self.sender.send(Some(header)).unwrap();
        }
    }
}

pub struct MasterchainLastBlockHeaderTracker {
    receiver: watch::Receiver<Option<BlockHeader>>,
    _cancellation_token: DropGuard,
}

impl MasterchainLastBlockHeaderTracker {
    pub fn new(client: LiteServerClient, last_block_tracker: MasterchainLastBlockTracker) -> Self {
        let cancellation_token = CancellationToken::new();
        let (sender, receiver) = watch::channel(None);

        CancellableActor::new(
            MasterchainLastBlockHeaderTrackerActor::new(client, last_block_tracker, sender),
            cancellation_token.clone(),
        )
        .spawn();

        Self {
            receiver,
            _cancellation_token: cancellation_token.drop_guard(),
        }
    }
}
