use std::task::{Context, Poll};
use tokio::sync::watch::Receiver;
use tower::{Service, ServiceExt};
use crate::client::LiteServerClient;
use crate::request::{Requestable, WithMasterChainSeqno};

struct WatchLastBlock {
    inner: LiteServerClient,
    last_seqno: Receiver<Option<i32>>,
}

impl WatchLastBlock {
    pub fn new(inner: LiteServerClient) -> Self {
        let (tx, rx) = tokio::sync::watch::channel(None);

        let mut tracker = inner.clone();

        tokio::spawn(async move {
            let mut last_seqno = (&mut tracker).oneshot(crate::tl::LiteServerGetMasterchainInfo {}).await.unwrap().last.seqno;
            tx.send(Some(last_seqno)).unwrap();
            tracing::info!(?last_seqno);

            loop {
                let request = WithMasterChainSeqno {
                    inner: crate::tl::LiteServerGetMasterchainInfo {},
                    seqno: last_seqno + 1,
                };

                match (&mut tracker).oneshot(request).await {
                    Ok(response) => {
                        tracing::info!(?response);
                        last_seqno = response.last.seqno;
                        tx.send(Some(last_seqno)).unwrap();
                    },
                    Err(error) => {
                        tracing::error!(?error);
                    }
                }
            }
        });

        Self { inner, last_seqno: rx }
    }
}

impl<R> Service<R> for WatchLastBlock where R: Requestable {
    type Response = R::Response;
    type Error = crate::client::Error;
    type Future = crate::client::ResponseFuture<R::Response>;

    fn poll_ready(&mut self, cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        match *self.last_seqno.borrow() {
            None => {
                cx.waker().wake_by_ref();

                Poll::Pending
            }
            Some(_) => { Poll::Ready(Ok(())) }
        }
    }

    fn call(&mut self, req: R) -> Self::Future {
        <LiteServerClient as Service<R>>::call(&mut self.inner, req)
    }
}

#[cfg(test)]
mod tests {
    use std::net::{Ipv4Addr, SocketAddrV4};
    use base64::Engine;
    use tower::ServiceExt;
    use tracing_test::traced_test;
    use adnl_tcp::client::ServerKey;
    use crate::client::Error;
    use crate::tl::{LiteServerError, LiteServerGetMasterchainInfo, LiteServerLookupBlock, TonNodeBlockId};
    use super::*;
    #[tokio::test]
    #[traced_test]
    #[ignore]
    async fn watch_last_block() -> anyhow::Result<()> {
        let mut client = crate::watch_last_block::tests::provided_client().await?;
        let last = (&mut client).oneshot(LiteServerGetMasterchainInfo {}).await?.last;
        let next_block_id = TonNodeBlockId { workchain: last.workchain, shard: last.shard, seqno: last.seqno + 10 };

        let response = client.oneshot(LiteServerLookupBlock {
            mode: 1,
            id: next_block_id,
            lt: None,
            utime: None
        }).await;

        tracing::info!(?response);
        assert!(response.is_err());
        let Error::LiteServerError(LiteServerError { code, .. }) = response.unwrap_err() else { unreachable!() };
        assert_eq!(code, 651);

        Ok(())
    }

    async fn provided_client() -> anyhow::Result<WatchLastBlock> {
        let ip: i32 = -2018135749;
        let ip = Ipv4Addr::from(ip as u32);
        let port = 53312;
        let key: ServerKey = base64::engine::general_purpose::STANDARD.decode("aF91CuUHuuOv9rm2W5+O/4h38M3sRm40DtSdRxQhmtQ=")?.as_slice().try_into()?;

        tracing::info!("Connecting to {}:{} with key {:?}", ip, port, key);

        let client = WatchLastBlock::new(LiteServerClient::connect(SocketAddrV4::new(ip, port), &key).await?);

        Ok(client)
    }
}
