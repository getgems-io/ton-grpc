use crate::client::Error;
use crate::tl::{
    LiteServerBlockData, LiteServerBlockHeader, LiteServerGetBlock, LiteServerLookupBlock,
};
use crate::tlb::block_header::BlockHeader;
use crate::tlb::merkle_proof::MerkleProof;
use crate::tracker::find_first_block::find_first_block_header;
use crate::tracker::masterchain_last_block_tracker::MasterchainLastBlockTracker;
use futures::TryFutureExt;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::watch;
use tokio::sync::watch::Ref;
use tokio_util::sync::{CancellationToken, DropGuard};
use ton_client_util::actor::cancellable_actor::CancellableActor;
use ton_client_util::actor::Actor;
use toner::tlb::bits::de::unpack_bytes;
use toner::ton::boc::BoC;
use tower::Service;

pub struct MasterchainFirstBlockTrackerActor<S> {
    client: S,
    last_block_tracker: MasterchainLastBlockTracker,
    sender: watch::Sender<Option<BlockHeader>>,
}

impl<S> MasterchainFirstBlockTrackerActor<S> {
    pub fn new(
        client: S,
        last_block_tracker: MasterchainLastBlockTracker,
        sender: watch::Sender<Option<BlockHeader>>,
    ) -> Self {
        Self {
            client,
            last_block_tracker,
            sender,
        }
    }
}

impl<S> Actor for MasterchainFirstBlockTrackerActor<S>
where
    S: Send + 'static,
    S: Service<
        LiteServerLookupBlock,
        Response = LiteServerBlockHeader,
        Error = Error,
        Future: Send,
    >,
    S: Service<LiteServerGetBlock, Response = LiteServerBlockData, Error = Error, Future: Send>,
{
    type Output = ();

    async fn run(mut self) -> <Self as Actor>::Output {
        let mut current_seqno = None;
        let last_block_id = self
            .last_block_tracker
            .wait_masterchain_info()
            .await
            .expect("expect to get masterchain info")
            .last;

        loop {
            let _ = find_first_block_header(
                &mut self.client,
                &last_block_id,
                current_seqno,
                current_seqno.map(|q| q + 1024),
            )
            .map_ok(|block| {
                current_seqno.replace(block.id.seqno);

                let boc: BoC = unpack_bytes(&block.header_proof).unwrap();
                let root = boc.single_root().unwrap();

                let header: MerkleProof = root.parse_fully().unwrap();

                let _ = self.sender.send(Some(header.virtual_root));
            })
            .inspect_err(|error| tracing::warn!(?error))
            .await;

            tokio::time::sleep(Duration::from_secs(60)).await;
        }
    }
}

#[derive(Debug, Clone)]
pub struct MasterchainFirstBlockTracker {
    receiver: watch::Receiver<Option<BlockHeader>>,
    _cancellation_token: Arc<DropGuard>,
}

impl MasterchainFirstBlockTracker {
    pub fn new<S>(client: S, last_block_tracker: MasterchainLastBlockTracker) -> Self
    where
        MasterchainFirstBlockTrackerActor<S>: Actor,
    {
        let cancellation_token = CancellationToken::new();
        let (sender, receiver) = watch::channel(None);

        CancellableActor::new(
            MasterchainFirstBlockTrackerActor::new(client, last_block_tracker, sender),
            cancellation_token.clone(),
        )
        .spawn();

        Self {
            receiver,
            _cancellation_token: Arc::new(cancellation_token.drop_guard()),
        }
    }

    pub fn borrow(&self) -> Ref<Option<BlockHeader>> {
        self.receiver.borrow()
    }

    pub fn receiver(&self) -> watch::Receiver<Option<BlockHeader>> {
        self.receiver.clone()
    }
}

#[cfg(test)]
mod test {
    use super::MasterchainFirstBlockTracker;
    use crate::client::tests::provided_client;
    use crate::tracker::masterchain_last_block_tracker::MasterchainLastBlockTracker;
    use tracing_test::traced_test;

    #[ignore]
    #[tokio::test]
    #[traced_test]
    async fn masterchain_first_block_tracker_delay() {
        let client = provided_client().await.unwrap();
        let last_tracker = MasterchainLastBlockTracker::new(client.clone());
        let first_tracker = MasterchainFirstBlockTracker::new(client, last_tracker);
        let mut prev_seqno = None;

        let mut receiver = first_tracker.receiver();

        for _ in 0..5 {
            receiver.changed().await.unwrap();

            let current_seqno = receiver.borrow().as_ref().unwrap().info.seq_no;
            println!("current_seqno = {current_seqno}");
            if let Some(seqno) = prev_seqno {
                assert!(current_seqno >= seqno);
            }
            prev_seqno.replace(current_seqno);
        }
    }
}
