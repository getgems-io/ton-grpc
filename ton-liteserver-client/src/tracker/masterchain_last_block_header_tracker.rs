use crate::tl::{LiteServerBoxedBlockHeader, LiteServerGetBlockHeader};
use crate::tlb::block_header::BlockHeader;
use crate::tlb::merkle_proof::MerkleProof;
use crate::tracker::masterchain_last_block_tracker::MasterchainLastBlockTracker;
use std::sync::Arc;
use tokio::sync::watch;
use tokio::sync::watch::Ref;
use tokio_util::sync::{CancellationToken, DropGuard};
use ton_client_util::actor::cancellable_actor::CancellableActor;
use ton_client_util::actor::Actor;
use toner::tlb::bits::de::unpack_bytes_fully;
use toner::ton::boc::BoC;
use tower::{Service, ServiceExt};

pub struct MasterchainLastBlockHeaderTrackerActor<S> {
    client: S,
    masterchain_info_tracker: MasterchainLastBlockTracker,
    sender: watch::Sender<Option<BlockHeader>>,
}

impl<S> MasterchainLastBlockHeaderTrackerActor<S> {
    pub fn new(
        client: S,
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

impl<S> Actor for MasterchainLastBlockHeaderTrackerActor<S>
where
    S: Send + 'static,
    S: Service<
        LiteServerGetBlockHeader,
        Response = LiteServerBoxedBlockHeader,
        Error = tower::BoxError,
        Future: Send,
    >,
{
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

            let Ok(response) = (&mut self.client)
                .oneshot(LiteServerGetBlockHeader::new(last_block_id))
                .await
            else {
                continue;
            };

            let header_bytes = response.header_proof;

            let boc: BoC = unpack_bytes_fully(&header_bytes).unwrap();
            let root = boc.single_root().unwrap();

            let header: MerkleProof = root.parse_fully().unwrap();

            self.sender.send(Some(header.virtual_root)).unwrap();
        }
    }
}

#[derive(Debug, Clone)]
pub struct MasterchainLastBlockHeaderTracker {
    receiver: watch::Receiver<Option<BlockHeader>>,
    _cancellation_token: Arc<DropGuard>,
}

impl MasterchainLastBlockHeaderTracker {
    pub fn new<S>(client: S, last_block_tracker: MasterchainLastBlockTracker) -> Self
    where
        MasterchainLastBlockHeaderTrackerActor<S>: Actor,
    {
        let cancellation_token = CancellationToken::new();
        let (sender, receiver) = watch::channel(None);

        CancellableActor::new(
            MasterchainLastBlockHeaderTrackerActor::new(client, last_block_tracker, sender),
            cancellation_token.clone(),
        )
        .spawn();

        Self {
            receiver,
            _cancellation_token: Arc::new(cancellation_token.drop_guard()),
        }
    }

    pub fn borrow(&self) -> Ref<'_, Option<BlockHeader>> {
        self.receiver.borrow()
    }
}
