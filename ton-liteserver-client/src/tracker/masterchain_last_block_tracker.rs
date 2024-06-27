use crate::request::WaitSeqno;
use crate::tl::{LiteServerGetMasterchainInfo, LiteServerMasterchainInfo};
use futures::future::Either;
use std::error::Error;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::watch;
use tokio::sync::watch::error::RecvError;
use tokio::sync::watch::Ref;
use tokio_util::sync::{CancellationToken, DropGuard};
use ton_client_util::actor::cancellable_actor::CancellableActor;
use ton_client_util::actor::Actor;
use tower::{Service, ServiceExt};

pub struct MasterchainLastBlockTrackerActor<S> {
    client: S,
    sender: watch::Sender<Option<LiteServerMasterchainInfo>>,
}

impl<S> MasterchainLastBlockTrackerActor<S> {
    pub fn new(client: S, sender: watch::Sender<Option<LiteServerMasterchainInfo>>) -> Self {
        Self { client, sender }
    }
}

impl<S, E> Actor for MasterchainLastBlockTrackerActor<S>
where
    E: Error,
    S: Send + 'static,
    S: Service<
        LiteServerGetMasterchainInfo,
        Response = LiteServerMasterchainInfo,
        Error = E,
        Future: Send,
    >,
    S: Service<
        WaitSeqno<LiteServerGetMasterchainInfo>,
        Response = LiteServerMasterchainInfo,
        Error = E,
        Future: Send,
    >,
{
    type Output = ();

    async fn run(mut self) {
        let mut current_seqno = None;

        loop {
            let response = match current_seqno {
                None => Either::Left(
                    (&mut self.client).oneshot(LiteServerGetMasterchainInfo::default()),
                ),
                Some(last) => Either::Right((&mut self.client).oneshot(WaitSeqno::with_timeout(
                    LiteServerGetMasterchainInfo::default(),
                    last + 1,
                    Duration::from_secs(10),
                ))),
            };

            match response.await {
                Ok(info) => {
                    current_seqno.replace(info.last.seqno);

                    let _ = self.sender.send(Some(info));
                }
                Err(error) => tracing::error!(?error),
            };
        }
    }
}

#[derive(Debug, Clone)]
pub struct MasterchainLastBlockTracker {
    receiver: watch::Receiver<Option<LiteServerMasterchainInfo>>,
    _cancellation_token: Arc<DropGuard>,
}

impl MasterchainLastBlockTracker {
    pub fn new<S>(client: S) -> Self
    where
        MasterchainLastBlockTrackerActor<S>: Actor,
    {
        let cancellation_token = CancellationToken::new();
        let (sender, receiver) = watch::channel(None);

        CancellableActor::new(
            MasterchainLastBlockTrackerActor::new(client, sender),
            cancellation_token.clone(),
        )
        .spawn();

        Self {
            receiver,
            _cancellation_token: Arc::new(cancellation_token.drop_guard()),
        }
    }

    pub fn borrow(&self) -> Ref<'_, Option<LiteServerMasterchainInfo>> {
        self.receiver.borrow()
    }

    pub fn receiver(&self) -> watch::Receiver<Option<LiteServerMasterchainInfo>> {
        self.receiver.clone()
    }

    pub async fn wait_masterchain_info(&mut self) -> Result<LiteServerMasterchainInfo, RecvError> {
        loop {
            if let Some(info) = self.receiver.borrow().as_ref() {
                return Ok(info.clone());
            }

            self.receiver.changed().await?;
        }
    }
}

#[cfg(test)]
mod test {
    use super::MasterchainLastBlockTracker;
    use crate::client::tests::provided_client;
    use tracing_test::traced_test;

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
