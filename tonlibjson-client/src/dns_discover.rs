use std::collections::HashSet;
use std::collections::hash_map::RandomState;
use std::net::IpAddr;
use std::pin::Pin;
use std::task::{Context, Poll};
use std::time::Duration;
use futures::never::Never;
use futures::Stream;
use tokio::sync::mpsc::UnboundedReceiver;
use tokio::time::MissedTickBehavior;
use tower::discover::Change;
use pin_project::pin_project;
use trust_dns_resolver::system_conf::read_system_conf;
use trust_dns_resolver::TokioAsyncResolver;
use crate::ton_config::{Liteserver, LiteserverId};

pub type DiscoverResult = Result<Change<String, Liteserver>, Never>;

#[pin_project]
pub struct DnsResolverDiscover {
    rx: UnboundedReceiver<DiscoverResult>
}

impl DnsResolverDiscover {
    pub fn new(host: &str, key: &str) -> Self {
        let (resolver_config, mut resolver_opts) = read_system_conf().unwrap();
        resolver_opts.positive_max_ttl = Some(Duration::from_secs(1));
        resolver_opts.negative_max_ttl = Some(Duration::from_secs(1));

        let resolver = TokioAsyncResolver::tokio(resolver_config, resolver_opts);
        let (tx, rx) = tokio::sync::mpsc::unbounded_channel();

        let host = host.to_owned();
        let key = key.to_owned();
        tokio::spawn(async move {
            let mut interval = tokio::time::interval(Duration::from_secs(2));
            interval.set_missed_tick_behavior(MissedTickBehavior::Skip);

            let mut state: HashSet<IpAddr, RandomState> = HashSet::default();
            loop {
                interval.tick().await;

                match resolver.lookup_ip(&host).await {
                    Ok(records) => {
                        let new_state = HashSet::from_iter(records.into_iter());

                        let new = new_state.difference(&state);
                        let drop = state.difference(&new_state);


                        for c in new {
                            let ip: u32 = match c {
                                IpAddr::V4(v4) => { (*v4).into() }
                                IpAddr::V6(_) => { unimplemented!("ipv6 is unimplemented") }
                            };

                            let ls = Liteserver {
                                id: LiteserverId {
                                    typ: "pub.ed25519".to_owned(),
                                    key: key.clone(),
                                },
                                ip: ip as i32,
                                port: 43679,
                            };

                            let _ = tx.send(Ok(Change::Insert(c.to_string(), ls)));
                        }
                        for c in drop { let _ = tx.send(Ok(Change::Remove(c.to_string()))); }

                        state = new_state;
                    },
                    Err(e) => {
                        tracing::error!(error =?e);
                    }
                }
            }
        });


        Self { rx }
    }
}


impl Stream for DnsResolverDiscover {
    type Item = DiscoverResult;

    fn poll_next(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        self.project().rx.poll_recv(cx)
    }
}
