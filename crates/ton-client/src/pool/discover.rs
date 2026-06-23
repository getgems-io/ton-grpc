use futures::{FutureExt, Stream};
use std::collections::HashSet;
use std::fmt::Debug;
use std::pin::Pin;
use std::task::{Context, Poll, ready};
use tokio::sync::mpsc;
use tokio_stream::StreamExt;
use ton_config::{LiteServer, LiteServerId, TonConfig};
use ton_tower::actor::{AbortOnDropHandle, Actor};
use tower::discover::Change;

#[derive(Debug, thiserror::Error)]
pub enum LiteServerDiscoverError {
    #[error("failed to load initial config: {0}")]
    InitialConfig(#[source] anyhow::Error),
    #[error("discover channel closed: {0}")]
    ChannelClosed(#[from] mpsc::error::SendError<Change<LiteServerId, TonConfig>>),
    #[error("discover actor join failed: {0}")]
    ActorJoin(#[from] tokio::task::JoinError),
}

pub struct LiteServerDiscoverHandle {
    receiver: mpsc::Receiver<Change<LiteServerId, TonConfig>>,
    join_handle: AbortOnDropHandle<Result<(), LiteServerDiscoverError>>,
    finished: bool,
}

impl LiteServerDiscoverHandle {
    pub fn new<S, E>(stream: S) -> Self
    where
        E: Into<anyhow::Error> + Debug + Send + 'static,
        S: Stream<Item = Result<TonConfig, E>> + Unpin + Send + 'static,
    {
        let (tx, rx) = mpsc::channel(100);
        let join_handle = LiteServerDiscoverActor {
            state: None,
            stream,
            tx,
        }
        .spawn_cancellable();

        Self {
            receiver: rx,
            join_handle,
            finished: false,
        }
    }
}

impl Stream for LiteServerDiscoverHandle {
    type Item = Result<Change<LiteServerId, TonConfig>, LiteServerDiscoverError>;

    fn poll_next(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        if self.finished {
            return Poll::Ready(None);
        }

        if let Some(change) = ready!(self.receiver.poll_recv(cx)) {
            return Poll::Ready(Some(Ok(change)));
        }

        let result = ready!(self.join_handle.poll_unpin(cx));
        self.finished = true;
        match result {
            Ok(Ok(())) => Poll::Ready(None),
            Ok(Err(config_error)) => Poll::Ready(Some(Err(config_error))),
            Err(join_error) => {
                Poll::Ready(Some(Err(LiteServerDiscoverError::ActorJoin(join_error))))
            }
        }
    }
}

struct LiteServerDiscoverActor<S> {
    state: Option<HashSet<LiteServer>>,
    stream: S,
    tx: mpsc::Sender<Change<LiteServerId, TonConfig>>,
}

impl<S, E> Actor for LiteServerDiscoverActor<S>
where
    E: Into<anyhow::Error> + Debug + Send,
    S: Stream<Item = Result<TonConfig, E>> + Unpin + Send + 'static,
{
    type Output = Result<(), LiteServerDiscoverError>;

    async fn run(mut self) -> <Self as Actor>::Output {
        while let Some(item) = self.stream.next().await {
            let config = match item {
                Ok(config) => config,
                Err(e) if self.state.is_none() => {
                    return Err(LiteServerDiscoverError::InitialConfig(e.into()));
                }
                Err(e) => {
                    tracing::error!("discover new config error: {:?}", e);
                    continue;
                }
            };

            tracing::info!("tick service discovery");

            let liteserver_new: HashSet<LiteServer> = config.liteservers.iter().cloned().collect();
            let state = self.state.take().unwrap_or_default();

            tracing::info!("discovered {} liteservers", liteserver_new.len());
            for ls in state.difference(&liteserver_new) {
                tracing::info!("remove {:?}", ls.id());
                self.tx.send(Change::Remove(ls.id.clone())).await?;
            }

            for ls in liteserver_new.difference(&state) {
                tracing::info!("insert {:?}", ls.id());

                self.tx
                    .send(Change::Insert(
                        ls.id.clone(),
                        config.with_liteserver(ls.clone()),
                    ))
                    .await?;
            }

            self.state = Some(liteserver_new);
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use anyhow::anyhow;
    use futures::StreamExt;
    use futures::stream;
    use std::convert::Infallible;
    use std::net::{Ipv4Addr, SocketAddrV4};
    use std::str::FromStr;

    #[tokio::test]
    async fn emits_insert_for_each_initial_liteserver() {
        let discover = LiteServerDiscoverHandle::new(snapshots(vec![config_with(&["a", "b"])]));

        let changes = collect(discover).await;

        assert_eq!(changes.len(), 2);
        assert!(matches!(&changes[0], Change::Insert(id, _) if id.key == "a"));
        assert!(matches!(&changes[1], Change::Insert(id, _) if id.key == "b"));
    }

    #[tokio::test]
    async fn emits_nothing_for_empty_config() {
        let discover = LiteServerDiscoverHandle::new(snapshots(vec![config_with(&[])]));

        let changes = collect(discover).await;

        assert!(changes.is_empty());
    }

    #[tokio::test]
    async fn emits_insert_only_for_new_liteserver() {
        let discover = LiteServerDiscoverHandle::new(snapshots(vec![
            config_with(&["a"]),
            config_with(&["a", "b"]),
        ]));

        let changes = collect(discover).await;

        assert_eq!(changes.len(), 2);
        assert!(matches!(&changes[0], Change::Insert(id, _) if id.key == "a"));
        assert!(matches!(&changes[1], Change::Insert(id, _) if id.key == "b"));
    }

    #[tokio::test]
    async fn emits_remove_for_dropped_liteserver() {
        let discover = LiteServerDiscoverHandle::new(snapshots(vec![
            config_with(&["a", "b"]),
            config_with(&["a"]),
        ]));

        let changes = collect(discover).await;

        assert_eq!(changes.len(), 3);
        assert!(matches!(&changes[0], Change::Insert(id, _) if id.key == "a"));
        assert!(matches!(&changes[1], Change::Insert(id, _) if id.key == "b"));
        assert!(matches!(&changes[2], Change::Remove(id) if id.key == "b"));
    }

    #[tokio::test]
    async fn emits_nothing_when_config_is_unchanged() {
        let discover = LiteServerDiscoverHandle::new(snapshots(vec![
            config_with(&["a"]),
            config_with(&["a"]),
        ]));

        let changes = collect(discover).await;

        assert_eq!(changes.len(), 1);
        assert!(matches!(&changes[0], Change::Insert(id, _) if id.key == "a"));
    }

    #[tokio::test]
    async fn ignores_error_after_first_success() {
        let discover = LiteServerDiscoverHandle::new(stream::iter(vec![
            Ok(config_with(&["a"])),
            Err(anyhow!("update failed")),
            Ok(config_with(&["a", "b"])),
        ]));

        let changes = collect(discover).await;

        assert_eq!(changes.len(), 2);
        assert!(matches!(&changes[0], Change::Insert(id, _) if id.key == "a"));
        assert!(matches!(&changes[1], Change::Insert(id, _) if id.key == "b"));
    }

    #[tokio::test]
    async fn errors_when_first_poll_errors() {
        let discover = LiteServerDiscoverHandle::new(stream::iter(vec![
            Err(anyhow!("initial load failed")),
            Ok(config_with(&["a"])),
        ]));

        let results: Vec<_> = discover.collect().await;

        assert_eq!(results.len(), 1);
        assert!(results[0].is_err());
    }

    async fn collect(discover: LiteServerDiscoverHandle) -> Vec<Change<LiteServerId, TonConfig>> {
        let mut changes: Vec<Change<LiteServerId, TonConfig>> =
            discover.map(|change| change.unwrap()).collect().await;

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
