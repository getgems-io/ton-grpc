use std::ops::Deref;
use std::sync::Arc;
use futures::future::Either;
use tokio::select;
use tokio::sync::watch;
use tokio::sync::watch::error::RecvError;
use tokio_util::sync::{CancellationToken, DropGuard};
use tower::ServiceExt;
use crate::client::LiteServerClient;
use crate::request::WaitSeqno;
use crate::tl::{LiteServerGetMasterchainInfo, LiteServerMasterchainInfo};

struct MasterchainLastBlockTrackerActor {
    client: LiteServerClient,
    sender: watch::Sender<Option<LiteServerMasterchainInfo>>,
    cancellation_token: CancellationToken
}

impl MasterchainLastBlockTrackerActor {
    pub fn new(client: LiteServerClient, sender: watch::Sender<Option<LiteServerMasterchainInfo>>, cancellation_token: CancellationToken) -> Self {
        Self { client, sender, cancellation_token }
    }

    pub fn run(mut self) {
        tokio::spawn(async move {
            let mut current_seqno = None;

            loop {
                let fut = match current_seqno {
                    None => Either::Left((&mut self.client)
                        .oneshot(LiteServerGetMasterchainInfo::default())),
                    Some(last) => Either::Right((&mut self.client)
                        .oneshot(WaitSeqno::new(LiteServerGetMasterchainInfo::default(), last + 1)))
                };

                select! {
                    _ = self.cancellation_token.cancelled() => {
                        tracing::error!("MasterChainLastBlockTrackerActor cancelled");
                        break;
                    }
                    response = fut => {
                        match response {
                            Ok(info) => {
                                current_seqno.replace(info.last.seqno);

                                let _ = self.sender.send(Some(info));
                            },
                            Err(error) => {
                                tracing::error!(?error);
                            }
                        }
                    }
                }
            }

            tracing::warn!("stop last block tracker actor");
        });
    }
}

#[derive(Debug, Clone)]
pub struct MasterchainLastBlockTracker {
    receiver: watch::Receiver<Option<LiteServerMasterchainInfo>>,
    _cancellation_token: Arc<DropGuard>
}

impl MasterchainLastBlockTracker {
    pub fn new(client: LiteServerClient) -> Self {
        let cancellation_token = CancellationToken::new();
        let (sender, receiver) = watch::channel(None);

        MasterchainLastBlockTrackerActor::new(client, sender, cancellation_token.clone()).run();

        Self { receiver, _cancellation_token: Arc::new(cancellation_token.drop_guard()) }
    }

    pub fn receiver(&self) -> watch::Receiver<Option<LiteServerMasterchainInfo>> {
        self.receiver.clone()
    }

    pub async fn wait_masterchain_info(&self) -> Result<LiteServerMasterchainInfo, RecvError> {
        let mut receiver = self.receiver.clone();
        loop {
            if let Some(info) = self.receiver.borrow().as_ref() {
                return Ok(info.clone());
            }

            receiver.changed().await?;
        }
    }
}

#[cfg(test)]
mod test {
    use tracing_test::traced_test;
    use crate::client::tests::provided_client;
    use super::MasterchainLastBlockTracker;

    #[ignore]
    #[tokio::test]
    #[traced_test]
    async fn masterchain_last_block_delay_test() {
        let client = provided_client().await.unwrap();
        let tracker = MasterchainLastBlockTracker::new(client);
        let mut prev_seqno = None;

        let mut receiver = tracker.receiver();

        for _ in 0..5 {
            receiver.changed().await.unwrap();

            let current_seqno = receiver.borrow().as_ref().unwrap().last.seqno;
            println!("current_seqno = {}", current_seqno);
            if let Some(seqno) = prev_seqno {
                assert!(current_seqno > seqno);
            }
            prev_seqno.replace(current_seqno);
        }
    }
}
