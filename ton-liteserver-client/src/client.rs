use std::future::Future;
use std::marker::PhantomData;
use std::net::SocketAddrV4;
use std::pin::Pin;
use std::sync::Arc;
use std::task::{Context, Poll};
use std::time::Duration;
use dashmap::DashMap;
use tower::Service;
use adnl_tcp::client::{AdnlTcpClient, ServerKey};
use futures::{ready, SinkExt, StreamExt};
use pin_project::pin_project;
use rand::random;
use thiserror::Error;
use tokio::sync::mpsc::UnboundedSender;
use tokio::sync::oneshot::Receiver;
use tokio::time::MissedTickBehavior;
use adnl_tcp::boxed::Boxed;
use adnl_tcp::types::{BareType, BoxedType};
use adnl_tcp::packet::Packet;
use adnl_tcp::ping::{is_pong_packet, ping_packet};
use adnl_tcp::deserializer::{Deserialize, from_bytes};
use adnl_tcp::serializer::to_bytes;
use crate::request::Requestable;
use crate::tl::{AdnlMessageAnswer, AdnlMessageQuery, Bytes, Int256, LiteServerError, LiteServerQuery};

#[derive(Error, Debug)]
pub enum Error {
    #[error("LiteServer error")]
    LiteServerError(#[from] LiteServerError),
    #[error("Deserialize error")]
    Deserialize,
    #[error("Inner channel is closed")]
    ChannelClosed,
    #[error("Response oneshot channel is closed")]
    OneshotClosed,
}

pub struct LiteserverClient {
    responses: Arc<DashMap<Int256, tokio::sync::oneshot::Sender<Bytes>>>,
    tx: UnboundedSender<AdnlMessageQuery>
}

impl LiteserverClient {
    pub async fn connect(addr: SocketAddrV4, server_key: &ServerKey) -> anyhow::Result<Self> {
        let inner = AdnlTcpClient::connect(addr, server_key).await?;
        let (mut write_half, mut read_half) = inner.split();
        let responses: Arc<DashMap<Int256, tokio::sync::oneshot::Sender<Bytes>>> = Arc::new(DashMap::new());

        let responses_read_half = responses.clone();
        tokio::spawn(async move {
            while let Some(response) = read_half.next().await {
                match response {
                    Ok(packet) if is_pong_packet(&packet) => {
                        tracing::trace!("pong packet received");
                    },
                    Ok(packet) => {
                        tracing::trace!(?packet);
                        let adnl_answer = from_bytes::<Boxed<AdnlMessageAnswer>>(packet.data)
                            .expect("expect adnl answer packet")
                            .unbox();

                        if let Some((_, oneshot)) = responses_read_half.remove(&adnl_answer.query_id) {
                            oneshot
                                .send(adnl_answer.answer)
                                .expect("expect oneshot alive");
                        }
                    }
                    Err(error) => {
                        tracing::error!(error = ?error, "reading error");

                        return
                    }
                }
            }
        });

        let (tx, rx) = tokio::sync::mpsc::unbounded_channel::<AdnlMessageQuery>();
        tokio::spawn(async move {
            let mut interval = tokio::time::interval(Duration::from_secs(5));
            interval.set_missed_tick_behavior(MissedTickBehavior::Skip);

            let stream = tokio_stream::wrappers::UnboundedReceiverStream::new(rx);
            let mut stream = tokio_stream::StreamExt::timeout_repeating(stream, interval);

            while let Some(request) = stream.next().await {
                match request {
                    Ok(adnl_query) => {
                        let data = to_bytes(&adnl_query.into_boxed());
                        write_half.send(Packet::new(data)).await.expect("expect to send adnl query packet")
                    }
                    Err(_) => {
                        write_half.send(ping_packet()).await.expect("expect to send ping packet")
                    }
                }
            }
        });


        Ok(Self { responses, tx })
    }
}

impl<R> Service<R> for LiteserverClient where R: Requestable + BoxedType, R::Response : BoxedType {
    type Response = R::Response;
    type Error = Error;
    type Future = ResponseFuture<R::Response>;

    fn poll_ready(&mut self, _: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        if self.tx.is_closed() {
            return Poll::Ready(Err(Error::ChannelClosed))
        }

        Poll::Ready(Ok(()))
    }

    fn call(&mut self, req: R) -> Self::Future {
        let data = to_bytes(&req);

        let query = LiteServerQuery { data };
        let query = to_bytes(&query.into_boxed());

        let query_id: Int256 = random();
        let request = AdnlMessageQuery { query_id, query };

        let (tx, rx) = tokio::sync::oneshot::channel();

        self.responses.insert(query_id, tx);
        if let Err(_) = self.tx.send(request) {
            return ResponseFuture::failed(Error::ChannelClosed);
        }

        ResponseFuture::new(query_id, rx, self.responses.clone())
    }
}


#[pin_project(project = ResponseStateProj)]
pub enum ResponseState {
    Failed { error: Option<Error> },
    Rx { #[pin] rx: Receiver<Bytes>, query_id: Int256, responses: Arc<DashMap<Int256, tokio::sync::oneshot::Sender<Bytes>>> }
}

#[pin_project(PinnedDrop)]
pub struct ResponseFuture<Response> {
    #[pin]
    state: ResponseState,

    _phantom: PhantomData<Response>,
}

impl<Response> ResponseFuture<Response> {
    fn new(query_id: Int256, rx: Receiver<Bytes>, responses: Arc<DashMap<Int256, tokio::sync::oneshot::Sender<Bytes>>>) -> Self {
        Self { state: ResponseState::Rx { query_id, responses, rx }, _phantom: PhantomData::default() }
    }

    fn failed(error: Error) -> Self {
        Self { state: ResponseState::Failed { error: Some(error) }, _phantom: PhantomData::default() }
    }
}

#[pin_project::pinned_drop]
impl<Response> PinnedDrop for ResponseFuture<Response> {
    fn drop(self: Pin<&mut Self>) {
        match self.state {
            ResponseState::Failed { .. } => {},
            ResponseState::Rx { ref responses, ref query_id, .. } => { responses.remove(query_id); }
        }
    }
}

impl<Response> Future for ResponseFuture<Response> where Response : BoxedType + Deserialize {
    type Output = Result<Response, Error>;

    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        let mut this = self.project();

        return match this.state.as_mut().project() {
            ResponseStateProj::Failed { error } => {
                Poll::Ready(Err(error.take().expect("polled after error")))
            },
            ResponseStateProj::Rx { rx, .. } => return match ready!(rx.poll(cx)) {
                Ok(response) => {
                    let response = from_bytes::<Result<Response, LiteServerError>>(response)
                        .map_err(|_| Error::Deserialize)?
                        .map_err(|e| Error::LiteServerError(e))?;

                    Poll::Ready(Ok(response))
                }
                Err(_) => {
                    Poll::Ready(Err(Error::OneshotClosed))
                }
            }
        }
    }
}


#[cfg(test)]
mod tests {
    use std::net::Ipv4Addr;
    use base64::Engine;
    use tower::ServiceExt;
    use tracing_test::traced_test;
    use std::time::SystemTime;
    use std::time::UNIX_EPOCH;
    use crate::tl::{LiteServerGetAllShardsInfo, LiteServerGetBlockProof, LiteServerGetMasterchainInfo, LiteServerGetMasterchainInfoExt, LiteServerGetVersion};
    use super::*;

    #[tokio::test]
    #[traced_test]
    async fn client_get_masterchain_info() -> anyhow::Result<()> {
        let client = provided_client().await?;

        let response = client.oneshot((LiteServerGetMasterchainInfo {}).into_boxed()).await?.unbox();

        assert_eq!(response.last.workchain, -1);
        assert_eq!(response.last.shard, -9223372036854775808);

        Ok(())
    }

    #[tokio::test]
    #[traced_test]
    async fn client_get_all_shards_info() -> anyhow::Result<()> {
        let mut client = provided_client().await?;
        let response = (&mut client).oneshot((LiteServerGetMasterchainInfo {}).into_boxed()).await?.unbox();

        let response = (&mut client).oneshot((LiteServerGetAllShardsInfo {
            id: response.last
        }).into_boxed()).await?.unbox();

        assert_eq!(response.id.workchain, -1);
        assert_eq!(response.id.shard, -9223372036854775808);

        Ok(())
    }

    #[tokio::test]
    #[traced_test]
    async fn client_get_version() -> anyhow::Result<()> {
        let client = provided_client().await?;

        let response = client.oneshot((LiteServerGetVersion {}).into_boxed()).await?.unbox();

        assert!(SystemTime::now().duration_since(UNIX_EPOCH)?.as_secs().abs_diff(response.now as u64) <= 10);

        Ok(())
    }

    #[tokio::test]
    #[traced_test]
    async fn client_error_test() -> anyhow::Result<()> {
        let client = provided_client().await?;

        let response = client.oneshot((LiteServerGetMasterchainInfoExt { mode: 1 }).into_boxed()).await;

        assert!(response.is_err());
        assert_eq!(response.unwrap_err().to_string(), "Error code: -400, message: \"unsupported getMasterchainInfo mode\"".to_owned());

        Ok(())
    }

    #[tokio::test]
    #[traced_test]
    async fn client_get_block_proof_test() -> anyhow::Result<()> {
        let mut client = provided_client().await?;
        let known_block = (&mut client).oneshot((LiteServerGetMasterchainInfo {}).into_boxed()).await?.unbox().last;

        let request = LiteServerGetBlockProof { mode: 0, known_block: known_block.clone(), target_block: None };
        let response = client.oneshot(request.into_boxed()).await?.unbox();

        assert_eq!(&response.from.seqno, &known_block.seqno);

        Ok(())
    }

    async fn provided_client() -> anyhow::Result<LiteserverClient> {
        let ip: i32 = -2018135749;
        let ip = Ipv4Addr::from(ip as u32);
        let port = 53312;
        let key: ServerKey = base64::engine::general_purpose::STANDARD.decode("aF91CuUHuuOv9rm2W5+O/4h38M3sRm40DtSdRxQhmtQ=")?.as_slice().try_into()?;

        tracing::info!("Connecting to {}:{} with key {:?}", ip, port, key);

        let client = LiteserverClient::connect(SocketAddrV4::new(ip, port), &key).await?;

        Ok(client)
    }
}
