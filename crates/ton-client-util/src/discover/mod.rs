use crate::actor::Actor;
use crate::actor::cancellable_actor::CancellableActor;
use futures::{Stream, StreamExt, TryStreamExt};
use std::collections::HashSet;
use std::convert::Infallible;
use std::path::PathBuf;
use std::pin::Pin;
use std::task::{Context, Poll, ready};
use tokio::sync::mpsc;
use tokio::time::Interval;
use tokio_stream::wrappers::IntervalStream;
use tokio_util::sync::{CancellationToken, DropGuard};
use ton_config::{LiteServer, LiteServerId, TonConfig, load_ton_config, read_ton_config};
use tower::discover::Change;
use url::Url;

pub fn read_ton_config_from_file_stream(
    path: PathBuf,
    interval: Interval,
) -> impl Stream<Item = Result<TonConfig, anyhow::Error>> {
    IntervalStream::new(interval)
        .map(move |_| path.clone())
        .then(read_ton_config)
}

pub fn read_ton_config_from_url_stream(
    url: Url,
    interval: Interval,
) -> impl Stream<Item = Result<TonConfig, anyhow::Error>> {
    IntervalStream::new(interval)
        .map(move |_| url.clone())
        .then(load_ton_config)
}

pub struct LiteServerDiscoverActor<S> {
    stream: S,
    sender: mpsc::Sender<Change<LiteServerId, TonConfig>>,
}

impl<S> LiteServerDiscoverActor<S> {
    pub fn new(stream: S, sender: mpsc::Sender<Change<LiteServerId, TonConfig>>) -> Self {
        Self { stream, sender }
    }
}

impl<S, E> Actor for LiteServerDiscoverActor<S>
where
    E: Send,
    S: Send + 'static,
    S: Stream<Item = Result<TonConfig, E>>,
{
    type Output = ();

    async fn run(self) -> <Self as Actor>::Output {
        let stream = self.stream;
        tokio::pin!(stream);

        let mut liteservers = HashSet::default();

        while let Ok(Some(new_config)) = stream.try_next().await {
            tracing::info!("tick service discovery");

            let liteserver_new: HashSet<LiteServer> =
                new_config.liteservers.iter().cloned().collect();

            let remove = liteservers
                .difference(&liteserver_new)
                .collect::<Vec<&LiteServer>>();
            let insert = liteserver_new
                .difference(&liteservers)
                .collect::<Vec<&LiteServer>>();

            tracing::info!(
                "Discovered {} liteservers, remove {}, insert {}",
                liteserver_new.len(),
                remove.len(),
                insert.len()
            );
            for ls in liteservers.difference(&liteserver_new) {
                tracing::info!("remove {:?}", ls.id());
                let _ = self.sender.send(Change::Remove(ls.id.clone())).await;
            }

            for ls in liteserver_new.difference(&liteservers) {
                tracing::info!("insert {:?}", ls.id());

                let _ = self
                    .sender
                    .send(Change::Insert(
                        ls.id.clone(),
                        new_config.with_liteserver(ls.clone()),
                    ))
                    .await;
            }

            liteservers.clone_from(&liteserver_new);
        }
    }
}

pub struct LiteServerDiscover {
    receiver: mpsc::Receiver<Change<LiteServerId, TonConfig>>,
    _drop_guard: DropGuard,
}

impl LiteServerDiscover {
    pub fn new<S>(stream: S) -> Self
    where
        LiteServerDiscoverActor<S>: Actor,
    {
        let token = CancellationToken::new();
        let (tx, rx) = mpsc::channel(100);
        CancellableActor::new(LiteServerDiscoverActor::new(stream, tx), token.clone()).spawn();

        Self {
            receiver: rx,
            _drop_guard: token.drop_guard(),
        }
    }
}

impl Stream for LiteServerDiscover {
    type Item = Result<Change<LiteServerId, TonConfig>, Infallible>;

    fn poll_next(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        let change = ready!(self.receiver.poll_recv(cx)).map(Ok);

        Poll::Ready(change)
    }
}
