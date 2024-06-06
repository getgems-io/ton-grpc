use std::ops::Deref;
use std::time::Duration;
use futures::future::Either;
use futures::try_join;
use tokio::select;
use tokio::sync::watch;
use tokio_util::sync::{CancellationToken, DropGuard};
use tower::ServiceExt;
use crate::client::{Error, LiteServerClient};
use crate::request::WaitSeqno;
use crate::tl::{LiteServerBlockData, LiteServerBlockHeader, LiteServerGetBlock, LiteServerGetBlockHeader, LiteServerGetMasterchainInfo, LiteServerLookupBlock, LiteServerMasterchainInfo, TonNodeBlockId, TonNodeBlockIdExt};
use crate::tracker::masterchain_last_block_tracker::MasterchainLastBlockTracker;

struct MasterchainFirstBlockTrackerActor {
    client: LiteServerClient,
    last_block_tracker: MasterchainLastBlockTracker,
    sender: watch::Sender<Option<LiteServerBlockHeader>>,
    cancellation_token: CancellationToken
}

impl MasterchainFirstBlockTrackerActor {
    pub fn new(client: LiteServerClient, last_block_tracker: MasterchainLastBlockTracker, sender: watch::Sender<Option<LiteServerBlockHeader>>, cancellation_token: CancellationToken) -> Self {
        Self { client, last_block_tracker, sender, cancellation_token }
    }

    pub fn run(mut self) {
        tokio::spawn(async move {
            let mut last_block_id = None;
            let mut current_seqno = None;

            loop {
                select! {
                    _ = self.cancellation_token.cancelled() => {
                        tracing::error!("MasterChainLastBlockTrackerActor cancelled");
                        break;
                    }
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
                        Self::find_first_blocks(
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

            tracing::warn!("stop first block tracker actor");
        });
    }

    async fn find_first_blocks(client: &mut LiteServerClient, start: &TonNodeBlockIdExt, lhs: Option<i32>, cur: Option<i32>) -> Result<LiteServerBlockHeader, Error> {
        let length = start.seqno;
        let mut rhs = length;
        let mut lhs = lhs.unwrap_or(1);
        let mut cur = cur.unwrap_or(start.seqno - 200000);

        let workchain = start.workchain;
        let shard = start.shard;

        let mut block = Self::check_block_available(client, TonNodeBlockId::new(workchain, shard, cur)).await;
        let mut success = None;

        let mut hops = 0;

        while lhs < rhs {
            // TODO[akostylev0] specify error
            if block.is_err() {
                lhs = cur + 1;
            } else {
                rhs = cur;
            }

            cur = (lhs + rhs) / 2;
            if cur == 0 { break; }

            block = Self::check_block_available(client, TonNodeBlockId::new(workchain, shard, cur)).await;
            if block.is_ok() {
                success = Some(block.as_ref().unwrap().clone());
            }

            hops += 1;
        }

        let delta = 4;
        let (header, _) = match block {
            Ok(b) => { b },
            Err(e) => match success {
                Some(b) if b.0.id.seqno - cur <= delta => { b },
                _ => { return Err(e) },
            }
        };

        tracing::trace!(hops = hops, seqno = header.id.seqno, "first seqno");

        Ok(header)
    }

    async fn check_block_available(client: &mut LiteServerClient, block_id: TonNodeBlockId) -> Result<(LiteServerBlockHeader, LiteServerBlockData), Error> {
        // TODO[akostylev0] research
        let block_header = client.oneshot(LiteServerLookupBlock::seqno(block_id)).await?;
        let block = client.oneshot(LiteServerGetBlock::new(block_header.id.clone())).await?;

        Ok((block_header, block))
    }

}

pub struct MasterchainFirstBlockTracker {
    receiver: watch::Receiver<Option<LiteServerBlockHeader>>,
    _cancellation_token: DropGuard
}

impl MasterchainFirstBlockTracker {
    pub fn new(client: LiteServerClient, last_block_tracker: MasterchainLastBlockTracker) -> Self {
        let cancellation_token = CancellationToken::new();
        let (sender, receiver) = watch::channel(None);

        MasterchainFirstBlockTrackerActor::new(client, last_block_tracker, sender, cancellation_token.clone()).run();

        Self { receiver, _cancellation_token: cancellation_token.drop_guard() }
    }

    pub fn receiver(&self) -> watch::Receiver<Option<LiteServerBlockHeader>> {
        self.receiver.clone()
    }
}

#[cfg(test)]
mod test {
    use tracing_test::traced_test;
    use crate::client::tests::provided_client;
    use crate::tracker::masterchain_last_block_tracker::MasterchainLastBlockTracker;
    use super::MasterchainFirstBlockTracker;

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
