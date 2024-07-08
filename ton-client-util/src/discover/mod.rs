use crate::actor::cancellable_actor::CancellableActor;
use crate::actor::Actor;
use crate::discover::config::{
    load_ton_config, read_ton_config, LiteServer, LiteServerId, TonConfig,
};
use futures::{Stream, StreamExt, TryStreamExt};
use hickory_resolver::error::ResolveError;
use hickory_resolver::system_conf::read_system_conf;
use hickory_resolver::TokioAsyncResolver;
use reqwest::Url;
use std::collections::HashSet;
use std::convert::Infallible;
use std::net::IpAddr;
use std::path::PathBuf;
use std::pin::Pin;
use std::task::{ready, Context, Poll};
use std::time::Duration;
use tokio::sync::mpsc;
use tokio::time::Interval;
use tokio_stream::wrappers::IntervalStream;
use tokio_util::sync::{CancellationToken, DropGuard};
use tower::discover::Change;

pub mod config;

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

        let dns = dns_resolver();
        let mut liteservers = HashSet::default();

        while let Ok(Some(new_config)) = stream.try_next().await {
            tracing::info!("tick service discovery");

            let mut liteserver_new: HashSet<LiteServer> = HashSet::default();
            for ls in new_config.liteservers.iter() {
                match apply_dns(dns.clone(), ls.clone()).await {
                    Err(e) => tracing::error!("dns error: {:?}", e),
                    Ok(ls) => {
                        liteserver_new.insert(ls);
                    }
                }
            }

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

fn dns_resolver() -> TokioAsyncResolver {
    let (resolver_config, mut resolver_opts) = read_system_conf().unwrap();
    resolver_opts.positive_max_ttl = Some(Duration::from_secs(1));
    resolver_opts.negative_max_ttl = Some(Duration::from_secs(1));

    TokioAsyncResolver::tokio(resolver_config, resolver_opts)
}

async fn apply_dns(
    dns_resolver: TokioAsyncResolver,
    ls: LiteServer,
) -> Result<LiteServer, ResolveError> {
    if let Some(host) = ls.host.as_ref() {
        let records = dns_resolver.lookup_ip(host).await?;

        for record in records {
            if let IpAddr::V4(ip) = record {
                return Ok(ls.with_ip(Into::<u32>::into(ip) as i32));
            }
        }
    }

    Ok(ls)
}
