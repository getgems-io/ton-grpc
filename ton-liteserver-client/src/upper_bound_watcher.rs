use futures::{StreamExt, TryStream, TryStreamExt};
use tokio::sync::watch::{Receiver, Ref};
use tokio_util::sync::{CancellationToken, DropGuard};
use tower::ServiceExt;
use crate::client::{Error, LiteServerClient};
use crate::request::WaitSeqno;
use crate::tl::{LiteServerBoxedMasterchainInfo, LiteServerGetMasterchainInfo, LiteServerLookupBlock, LiteServerMasterchainInfo, TonNodeBlockId, TonNodeBlockIdExt};

pub struct UpperBoundWatcher {
    current: Receiver<Option<LiteServerMasterchainInfo>>,
    join_handle: tokio::task::JoinHandle<()>,
    _drop_guard: DropGuard,
}

impl UpperBoundWatcher {
    pub fn new(inner: LiteServerClient) -> Self {
        let (tx, current) = tokio::sync::watch::channel(None);
        let token = CancellationToken::new();
        let join_handle = tokio::spawn({
            let token = token.child_token();
            let mut stream = upper_bound_stream(inner);

            async move {
                loop {
                    tokio::select! {
                        _ = token.cancelled() => {
                            tracing::error!("UpperBoundWatcher cancelled");
                            break;
                        },
                        Ok(Some(block_id)) = stream.try_next() => {
                            let _ = tx.send(Some(block_id)).inspect_err(|e| tracing::error!(error = ?e));
                        }
                    }
                }
            }
        });

        Self { current, join_handle, _drop_guard: token.drop_guard() }
    }

    pub fn current_upper_bound(&self) -> Ref<'_, Option<LiteServerMasterchainInfo>> {
        self.current.borrow()
    }
}

fn upper_bound_stream(client: LiteServerClient) -> impl TryStream<Ok=LiteServerMasterchainInfo, Error=crate::client::Error> + Unpin {
    struct State {
        client: LiteServerClient,
        current: Option<LiteServerMasterchainInfo>
    }

    futures::stream::try_unfold(State { client, current: None }, |State { mut client, current }| async move {
        match current {
            None => {
                let info = (&mut client).oneshot(LiteServerGetMasterchainInfo::default()).await?;

                Ok(Some((info.clone(), State { client, current: Some(info) })))
            },
            Some(mut current) => {
                let mut next_block_id: TonNodeBlockId = current.last.clone().into();
                next_block_id.seqno += 1;
                let next_seqno = next_block_id.seqno;

                let request = WaitSeqno::new(LiteServerLookupBlock { mode: 1, id: next_block_id, lt: None, utime: None  }, next_seqno);

                loop {
                    let block_id = (&mut client).oneshot(request.clone()).await;

                    match block_id {
                        Ok(block_id) => {
                            current.last = block_id.id;

                            return Ok(Some((current.clone(), State { client, current: Some(current) })))
                        }
                        // timeout
                        Err(Error::LiteServerError(crate::tl::LiteServerError { code: 652, .. })) => {
                            continue;
                        },
                        Err(e) => return Err(e)
                    }
                }
            }
        }
    }).boxed()
}


#[cfg(test)]
mod tests {
    use tracing_test::traced_test;
    use super::*;

    #[tokio::test]
    #[traced_test]
    #[ignore]
    async fn upper_bound_stream_test() -> anyhow::Result<()> {
        let client = crate::client::tests::provided_client().await?;
        let mut stream = upper_bound_stream(client);

        let block_id1 = stream.try_next().await.unwrap().unwrap();
        tracing::info!("{:?}", block_id1);
        let block_id2 = stream.try_next().await.unwrap().unwrap();
        tracing::info!("{:?}", block_id2);
        let block_id3 = stream.try_next().await.unwrap().unwrap();
        tracing::info!("{:?}", block_id3);

        assert!(block_id1.last.seqno < block_id2.last.seqno);
        assert!(block_id2.last.seqno < block_id3.last.seqno);
        Ok(())
    }

}
