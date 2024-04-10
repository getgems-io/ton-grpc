use std::future::Future;
use std::net::SocketAddrV4;
use std::pin::Pin;
use std::sync::Arc;
use std::task::{Context, Poll};
use std::time::Duration;
use anyhow::{anyhow, Error};
use dashmap::DashMap;
use tower::Service;
use adnl_tcp::client::{AdnlTcpClient, ServerKey};
use futures::sink::Sink;
use futures::{FutureExt, SinkExt, StreamExt};
use futures::stream::SplitSink;
use rand::random;
use tokio::sync::mpsc::UnboundedSender;
use tokio::time::MissedTickBehavior;
use adnl_tcp::packet::Packet;
use adnl_tcp::ping::{is_pong_packet, ping_packet};
use crate::deserializer::from_bytes;
use crate::request::Requestable;
use crate::serializer::to_bytes;
use crate::tl::{AdnlMessageAnswer, AdnlMessageQuery, Bytes, Int256, LiteServerQuery};

pub struct LiteServerClient {
    responses: Arc<DashMap<Int256, tokio::sync::oneshot::Sender<Bytes>>>,
    tx: UnboundedSender<AdnlMessageQuery>
}

impl LiteServerClient {
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
                        let adnl_answer = from_bytes::<AdnlMessageAnswer>(packet.data)
                            .expect("expect adnl answer packet");

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
                        let data = to_bytes(&adnl_query).expect("expect to serialize adnl query");
                        write_half.send(Packet::new(&data)).await.expect("expect to send adnl query packet")
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

impl<R : Requestable> Service<R> for LiteServerClient {
    type Response = R::Response;
    type Error = anyhow::Error;
    type Future = Pin<Box<dyn Future<Output = Result<Self::Response, Self::Error>> + Send>>;

    fn poll_ready(&mut self, cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        if self.tx.is_closed() {
            return Poll::Ready(Err(anyhow!("inner channel is closed")))
        }

        Poll::Ready(Ok(()))
    }

    fn call(&mut self, req: R) -> Self::Future {
        let Ok(data) = to_bytes(&req) else {
            return std::future::ready(Err(anyhow!("cannot serialize request"))).boxed()
        };

        let query = LiteServerQuery { data };
        let Ok(query) = to_bytes(&query) else {
            return std::future::ready(Err(anyhow!("cannot serialize liteserver query"))).boxed()
        };

        let query_id: Int256 = random();
        let request = AdnlMessageQuery { query_id, query };

        let (tx, rx) = tokio::sync::oneshot::channel();

        self.responses.insert(query_id, tx);
        self.tx.send(request).expect("inner channel is closed");

        return async {
            let response = rx.await?;

            let response = from_bytes::<R::Response>(response)?;

            return Ok(response)
        }.boxed()
    }
}

#[cfg(test)]
mod tests {
    use std::net::Ipv4Addr;
    use base64::Engine;
    use tower::ServiceExt;
    use tracing_test::traced_test;
    use crate::tl::LiteServerGetMasterchainInfo;
    use super::*;

    #[tokio::test]
    #[traced_test]
    async fn client_get_masterchain_info() -> anyhow::Result<()> {
        let client = provided_client().await?;

        let response = client.oneshot(LiteServerGetMasterchainInfo {}).await?;

        assert_eq!(response.last.workchain, -1);
        assert_eq!(response.last.shard, -9223372036854775808);

        Ok(())
    }

    async fn provided_client() -> anyhow::Result<LiteServerClient> {
        let ip: i32 = -2018147075;
        let ip = Ipv4Addr::from(ip as u32);
        let port = 46529;
        let key: ServerKey = base64::engine::general_purpose::STANDARD.decode("jLO6yoooqUQqg4/1QXflpv2qGCoXmzZCR+bOsYJ2hxw=")?.as_slice().try_into()?;

        tracing::info!("Connecting to {}:{} with key {:?}", ip, port, key);

        let client = LiteServerClient::connect(SocketAddrV4::new(ip, port), &key).await?;

        Ok(client)
    }
}
