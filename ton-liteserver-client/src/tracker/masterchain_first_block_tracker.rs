use crate::client::LiteServerClient;
use crate::tl::LiteServerBlockHeader;
use crate::tracker::find_first_block::find_first_block_header;
use crate::tracker::masterchain_last_block_tracker::MasterchainLastBlockTracker;
use std::time::Duration;
use tokio::select;
use tokio::sync::watch;
use tokio_util::sync::{CancellationToken, DropGuard};
use ton_client_utils::actor::cancellable_actor::CancellableActor;
use ton_client_utils::actor::Actor;

struct MasterchainFirstBlockTrackerActor {
    client: LiteServerClient,
    last_block_tracker: MasterchainLastBlockTracker,
    sender: watch::Sender<Option<LiteServerBlockHeader>>,
}

impl MasterchainFirstBlockTrackerActor {
    pub fn new(
        client: LiteServerClient,
        last_block_tracker: MasterchainLastBlockTracker,
        sender: watch::Sender<Option<LiteServerBlockHeader>>,
    ) -> Self {
        Self {
            client,
            last_block_tracker,
            sender,
        }
    }
}

impl Actor for MasterchainFirstBlockTrackerActor {
    type Output = ();

    async fn run(mut self) -> <Self as Actor>::Output {
        let mut last_block_id = None;
        let mut current_seqno = None;

        loop {
            select! {
                result = async {
                    self.last_block_tracker.wait_masterchain_info().await
                }, if last_block_id.is_none() => {
                    match result {
                        Ok(masterchain_info) => {
                            last_block_id.replace(masterchain_info.last);
                        },
                        Err(error) => {
                            tracing::error!(?error);
                        }
                    }
                },
                result = async {
                    find_first_block_header(
                        &mut self.client,
                        last_block_id.as_ref().unwrap(),
                        current_seqno,
                        current_seqno.map(|q| q + 32)
                    ).await
                }, if last_block_id.is_some() => {
                    match result {
                        Ok(block) => {
                            current_seqno.replace(block.id.seqno);
                            self.sender.send(Some(block)).unwrap();

                            tokio::time::sleep(Duration::from_secs(30)).await;
                        }
                        Err(error) => {
                            tracing::error!(?error);
                        }
                    }
                }
            }
        }
    }
}

pub struct MasterchainFirstBlockTracker {
    receiver: watch::Receiver<Option<LiteServerBlockHeader>>,
    _cancellation_token: DropGuard,
}

impl MasterchainFirstBlockTracker {
    pub fn new(client: LiteServerClient, last_block_tracker: MasterchainLastBlockTracker) -> Self {
        let cancellation_token = CancellationToken::new();
        let (sender, receiver) = watch::channel(None);

        CancellableActor::new(
            MasterchainFirstBlockTrackerActor::new(client, last_block_tracker, sender),
            cancellation_token.clone(),
        )
        .spawn();

        Self {
            receiver,
            _cancellation_token: cancellation_token.drop_guard(),
        }
    }

    pub fn receiver(&self) -> watch::Receiver<Option<LiteServerBlockHeader>> {
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

            let current_seqno = receiver.borrow().as_ref().unwrap().id.seqno;
            println!("current_seqno = {}", current_seqno);
            if let Some(seqno) = prev_seqno {
                assert!(current_seqno >= seqno);
            }
            prev_seqno.replace(current_seqno);
        }
    }
}
