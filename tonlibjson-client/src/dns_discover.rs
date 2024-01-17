use std::collections::{HashMap, HashSet};
use std::collections::hash_map::RandomState;
use std::net::IpAddr;
use std::time::Duration;
use tokio::time::{Interval, MissedTickBehavior};
use trust_dns_resolver::system_conf::read_system_conf;
use trust_dns_resolver::TokioAsyncResolver;

pub(crate) struct DnsResolverDiscover {}

impl DnsResolverDiscover {
    pub fn new(url: &str) -> Self {
        let (resolver_config, mut resolver_opts) = read_system_conf().unwrap();
        resolver_opts.positive_max_ttl = Some(Duration::from_secs(1));
        resolver_opts.negative_max_ttl = Some(Duration::from_secs(1));

        tracing::debug!(resolver_config = ?resolver_config, resolver_opts = ?resolver_opts);

        let resolver = TokioAsyncResolver::tokio(resolver_config, resolver_opts);

        let url = url.to_owned();
        tokio::spawn(async move {
            let mut interval = tokio::time::interval(Duration::from_secs(2));
            interval.set_missed_tick_behavior(MissedTickBehavior::Skip);


            let mut state: HashSet<IpAddr, RandomState> = HashSet::default();
            loop {
                interval.tick().await;

                let records = resolver.lookup_ip(&url).await.unwrap();

                let new_state = HashSet::from_iter(records.into_iter());

                let new = new_state.difference(&state);
                let drop = state.difference(&new_state);


                let state = new_state;
            }
        });


        Self {  }
    }
}
