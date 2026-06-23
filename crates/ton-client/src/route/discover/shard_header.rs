use crate::RequestHandler;
use crate::route::registry::Registry;
use std::sync::Arc;
use tokio::sync::mpsc;
use tokio_retry::Retry;
use tokio_retry::strategy::{FibonacciBackoff, jitter};
use ton_tower::actor::{AbortOnDropHandle, Actor};
use ton_tower::request::GetBlockHeader;
use ton_tower::response::BlockIdExt;
use tower::ServiceExt;

pub struct ShardHeaderActorHandle {
    tx: mpsc::UnboundedSender<BlockIdExt>,
    _handle: AbortOnDropHandle<()>,
}

impl ShardHeaderActorHandle {
    pub fn new<S>(registry: Arc<Registry>, client: S) -> Self
    where
        S: RequestHandler<GetBlockHeader> + Clone + Sync + 'static,
    {
        let (tx, rx) = mpsc::unbounded_channel::<BlockIdExt>();

        let handle = ShardHeaderActor::new(rx, registry, client).spawn_cancellable();

        Self {
            tx,
            _handle: handle,
        }
    }

    pub fn send(&self, block_id: BlockIdExt) -> anyhow::Result<()> {
        self.tx.send(block_id).map_err(Into::into)
    }
}

struct ShardHeaderActor<S> {
    rx: mpsc::UnboundedReceiver<BlockIdExt>,
    registry: Arc<Registry>,
    client: S,
}

impl<S> ShardHeaderActor<S> {
    fn new(rx: mpsc::UnboundedReceiver<BlockIdExt>, registry: Arc<Registry>, client: S) -> Self {
        Self {
            rx,
            registry,
            client,
        }
    }
}

impl<S> Actor for ShardHeaderActor<S>
where
    S: RequestHandler<GetBlockHeader> + Clone + Sync + 'static,
{
    type Output = ();

    async fn run(mut self) -> Self::Output {
        while let Some(block_id) = self.rx.recv().await {
            let retry_strategy = FibonacciBackoff::from_millis(32).map(jitter).take(16);
            match Retry::start(retry_strategy, || {
                let client = self.client.clone();
                let block_id = block_id.clone();
                client.oneshot(GetBlockHeader { id: block_id })
            })
            .await
            {
                Ok(header) => self.registry.upsert_right(&header),
                Err(e) => {
                    tracing::warn!(error = ?e, "failed to get shard header");
                }
            }
        }
    }
}

#[cfg(test)]
mod integration {
    use super::*;
    use crate::route::{Seqno, ShardId};
    use crate::{Client, TonService};
    use rstest::{fixture, rstest};
    use std::time::Duration;
    use testcontainers_ton::LocalLiteServer;
    use ton_liteserver_client::adapter::LiteServerAdapter;
    use ton_liteserver_client::client::LiteServerClient;
    use ton_tower::request::GetMasterchainInfo;
    use tonlibjson_client::{MakeTonlibjsonAdapter, TonlibjsonAdapter};
    use tracing_test::traced_test;

    #[fixture]
    async fn server() -> LocalLiteServer {
        LocalLiteServer::new().await.unwrap()
    }

    #[rstest]
    #[case::tonlibjson(tonlibjson_client)]
    #[case::liteserver(liteserver_client)]
    #[tokio::test]
    #[traced_test]
    async fn should_store_shard_header<S, F>(
        #[future(awt)] server: LocalLiteServer,
        #[case] make_client: F,
    ) where
        S: TonService,
        F: AsyncFnOnce(LocalLiteServer) -> (LocalLiteServer, S),
    {
        let (_server, client) = make_client(server).await;
        let block_id = client
            .clone()
            .oneshot(GetMasterchainInfo::default())
            .await
            .unwrap()
            .last;
        let shard_id = (block_id.workchain, block_id.shard);
        let expected_seqno = block_id.seqno;
        let registry = Arc::new(Registry::default());
        let handle = ShardHeaderActorHandle::new(registry.clone(), client);

        handle.send(block_id).unwrap();
        let seqno = wait_for_seqno(&registry, &shard_id).await;

        assert_eq!(expected_seqno, seqno);
    }

    async fn tonlibjson_client(
        server: LocalLiteServer,
    ) -> (LocalLiteServer, Client<TonlibjsonAdapter>) {
        let adapter = MakeTonlibjsonAdapter
            .oneshot(server.config().clone())
            .await
            .unwrap();

        (server, Client::new(adapter))
    }

    async fn liteserver_client(
        server: LocalLiteServer,
    ) -> (LocalLiteServer, Client<LiteServerAdapter>) {
        let inner = LiteServerClient::connect(server.addr(), server.server_key())
            .await
            .unwrap();

        (server, Client::new(LiteServerAdapter::new(inner)))
    }

    async fn wait_for_seqno(registry: &Registry, shard_id: &ShardId) -> Seqno {
        for _ in 0..50 {
            if let Some(seqno) = registry.get_last_seqno(shard_id) {
                return seqno;
            }
            tokio::time::sleep(Duration::from_millis(100)).await;
        }

        panic!("shard header was not stored in registry within timeout");
    }
}
