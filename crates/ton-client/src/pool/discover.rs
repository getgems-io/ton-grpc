use futures::{Stream, stream};
use std::collections::HashSet;
use std::fmt::Debug;
use std::pin::Pin;
use std::task::{Context, Poll, ready};
use tokio::sync::mpsc;
use tokio_stream::StreamExt;
use ton_config::{LiteServer, LiteServerId, TonConfig};
use ton_tower::actor::{AbortOnDropHandle, Actor};
use tower::discover::Change;

pub struct LiteServerDiscoverHandle {
    receiver: mpsc::Receiver<Change<LiteServerId, TonConfig>>,
    _join_handle: AbortOnDropHandle<anyhow::Result<()>>,
}

impl LiteServerDiscoverHandle {
    pub fn new<S, E>(initial: TonConfig, update: S) -> Self
    where
        E: Debug + Send + 'static,
        S: Stream<Item = Result<TonConfig, E>> + Unpin + Send + 'static,
    {
        let (tx, rx) = mpsc::channel(100);
        let stream = stream::iter([Ok(initial)]).chain(update);
        let join_handle = LiteServerDiscoverActor {
            state: HashSet::default(),
            stream,
            tx,
        }
        .spawn_cancellable();

        Self {
            receiver: rx,
            _join_handle: join_handle,
        }
    }
}

impl Stream for LiteServerDiscoverHandle {
    type Item = Change<LiteServerId, TonConfig>;

    fn poll_next(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        if let Some(change) = ready!(self.receiver.poll_recv(cx)) {
            return Poll::Ready(Some(change));
        }

        Poll::Ready(None)
    }
}

struct LiteServerDiscoverActor<S> {
    state: HashSet<LiteServer>,
    stream: S,
    tx: mpsc::Sender<Change<LiteServerId, TonConfig>>,
}

impl<S, E> Actor for LiteServerDiscoverActor<S>
where
    E: Debug + Send,
    S: Stream<Item = Result<TonConfig, E>> + Unpin + Send + 'static,
{
    type Output = anyhow::Result<()>;

    async fn run(mut self) -> <Self as Actor>::Output {
        while let Some(item) = self.stream.next().await {
            let config = match item {
                Ok(config) => config,
                Err(e) => {
                    tracing::error!("discover new config error: {:?}", e);
                    continue;
                }
            };

            tracing::info!("tick service discovery");

            let liteserver_new: HashSet<LiteServer> = config.liteservers.iter().cloned().collect();

            tracing::info!("discovered {} liteservers", liteserver_new.len());
            for ls in self.state.difference(&liteserver_new) {
                tracing::info!("remove {:?}", ls.id());
                self.tx.send(Change::Remove(ls.id.clone())).await?;
            }

            for ls in liteserver_new.difference(&self.state) {
                tracing::info!("insert {:?}", ls.id());

                self.tx
                    .send(Change::Insert(
                        ls.id.clone(),
                        config.with_liteserver(ls.clone()),
                    ))
                    .await?;
            }

            self.state = liteserver_new;
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use futures::StreamExt;
    use futures::stream;
    use std::convert::Infallible;
    use std::net::{Ipv4Addr, SocketAddrV4};
    use std::str::FromStr;

    #[tokio::test]
    async fn emits_insert_for_each_initial_liteserver() {
        let discover = LiteServerDiscoverHandle::new(config_with(&["a", "b"]), snapshots(vec![]));

        let changes = collect(discover).await;

        assert_eq!(changes.len(), 2);
        assert!(matches!(&changes[0], Change::Insert(id, _) if id.key == "a"));
        assert!(matches!(&changes[1], Change::Insert(id, _) if id.key == "b"));
    }

    #[tokio::test]
    async fn emits_nothing_for_empty_config() {
        let discover = LiteServerDiscoverHandle::new(config_with(&[]), snapshots(vec![]));

        let changes = collect(discover).await;

        assert!(changes.is_empty());
    }

    #[tokio::test]
    async fn emits_insert_only_for_new_liteserver() {
        let discover = LiteServerDiscoverHandle::new(
            config_with(&["a"]),
            snapshots(vec![config_with(&["a", "b"])]),
        );

        let changes = collect(discover).await;

        assert_eq!(changes.len(), 2);
        assert!(matches!(&changes[0], Change::Insert(id, _) if id.key == "a"));
        assert!(matches!(&changes[1], Change::Insert(id, _) if id.key == "b"));
    }

    #[tokio::test]
    async fn emits_remove_for_dropped_liteserver() {
        let discover = LiteServerDiscoverHandle::new(
            config_with(&["a", "b"]),
            snapshots(vec![config_with(&["a"])]),
        );

        let changes = collect(discover).await;

        assert_eq!(changes.len(), 3);
        assert!(matches!(&changes[0], Change::Insert(id, _) if id.key == "a"));
        assert!(matches!(&changes[1], Change::Insert(id, _) if id.key == "b"));
        assert!(matches!(&changes[2], Change::Remove(id) if id.key == "b"));
    }

    #[tokio::test]
    async fn emits_nothing_when_config_is_unchanged() {
        let discover = LiteServerDiscoverHandle::new(
            config_with(&["a"]),
            snapshots(vec![config_with(&["a"])]),
        );

        let changes = collect(discover).await;

        assert_eq!(changes.len(), 1);
        assert!(matches!(&changes[0], Change::Insert(id, _) if id.key == "a"));
    }

    async fn collect(discover: LiteServerDiscoverHandle) -> Vec<Change<LiteServerId, TonConfig>> {
        let mut changes: Vec<Change<LiteServerId, TonConfig>> = discover.collect().await;

        changes.sort_by_key(|change| match change {
            Change::Insert(id, _) => (0, id.key.clone()),
            Change::Remove(id) => (1, id.key.clone()),
        });

        changes
    }

    fn snapshots(
        configs: Vec<TonConfig>,
    ) -> impl Stream<Item = Result<TonConfig, Infallible>> + Unpin {
        stream::iter(configs.into_iter().map(Ok::<_, Infallible>))
    }

    fn config_with(keys: &[&str]) -> TonConfig {
        let mut config =
            TonConfig::from_str(r#"{"@type":"config.global","liteservers":[]}"#).unwrap();

        config.liteservers = keys.iter().map(|key| liteserver(key)).collect();

        config
    }

    fn liteserver(key: &str) -> LiteServer {
        LiteServer::new(
            LiteServerId { key: key.into() },
            SocketAddrV4::new(Ipv4Addr::LOCALHOST, 0),
        )
    }
}
