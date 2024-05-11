use futures::{StreamExt, TryStream, TryStreamExt};
use tokio::sync::watch::{Receiver, Ref};
use tokio_util::sync::{CancellationToken, DropGuard};
use tower::ServiceExt;
use crate::client::{Error, LiteServerClient};
use crate::request::WaitSeqno;
use crate::tl::{LiteServerGetMasterchainInfo, LiteServerLookupBlock, LiteServerMasterchainInfo, TonNodeBlockId};

pub struct UpperBoundWatcher {
    current: Receiver<Option<LiteServerMasterchainInfo>>,
    _drop_guard: DropGuard,
}

impl UpperBoundWatcher {
    pub fn new(mut inner: LiteServerClient) -> Self {
        let (tx, current) = tokio::sync::watch::channel(None);
        let token = CancellationToken::new();
        tokio::spawn({
            let token = token.child_token();

            async move {
                loop {
                    let mut stream = upper_bound_stream(&mut inner);
                    'inner: loop {
                        tokio::select! {
                            _ = token.cancelled() => {
                                tracing::error!("UpperBoundWatcher cancelled");
                                break;
                            },
                            result = stream.try_next() => {
                                match result {
                                    Ok(Some(block_id)) => {
                                        let _ = tx.send(Some(block_id))
                                        .inspect_err(|error| tracing::error!(?error));
                                    },
                                    Ok(None) => { unreachable!(); }
                                    Err(error) => { tracing::error!(?error); break 'inner; },
                                }
                            }
                        }
                    }
                }
            }
        });

        Self { current, _drop_guard: token.drop_guard() }
    }

    pub fn current_upper_bound(&self) -> Ref<'_, Option<LiteServerMasterchainInfo>> {
        self.current.borrow()
    }
}

fn upper_bound_stream<'a>(client: &'a mut LiteServerClient) -> impl TryStream<Ok=LiteServerMasterchainInfo, Error=crate::client::Error> + Unpin + 'a {
    struct State<'a> {
        client: &'a mut LiteServerClient,
        current: Option<LiteServerMasterchainInfo>
    }

    futures::stream::try_unfold(State { client, current: None }, |State { client, current }| async move {
        match current {
            None => {
                let info = client.oneshot(LiteServerGetMasterchainInfo::default()).await?;

                Ok(Some((info.clone(), State { client, current: Some(info) })))
            },
            Some(mut current) => {
                let mut next_block_id: TonNodeBlockId = current.last.clone().into();
                next_block_id.seqno += 1;
                let next_seqno = next_block_id.seqno;

                let request = WaitSeqno::new(LiteServerLookupBlock { mode: 1, id: next_block_id, lt: None, utime: None  }, next_seqno);

                loop {
                    let block_id = client.oneshot(request.clone()).await;

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
        let mut client = crate::client::tests::provided_client().await?;
        let mut stream = upper_bound_stream(&mut client);

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
