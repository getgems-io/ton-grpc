use std::net::{IpAddr, SocketAddr};
use std::str::FromStr;
use std::time::Duration;
use trust_dns_resolver::config::{NameServerConfig, NameServerConfigGroup, Protocol, ResolverConfig, ResolverOpts};
use trust_dns_resolver::{Name, TokioAsyncResolver};
use trust_dns_resolver::proto::rr::RecordType;
use trust_dns_resolver::system_conf::read_system_conf;
use tonlibjson_client::ton::{TonClient, TonClientBuilder};

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt::init();
    let ip = IpAddr::from_str("127.0.0.1").unwrap();

    let nsc = NameServerConfig {
        socket_addr: SocketAddr::new(ip, 5300),
        protocol: Protocol::Tcp,
        tls_dns_name: None,
        trust_negative_responses: true,
        bind_addr: None,
    };

    let resolver = TokioAsyncResolver::tokio(
        ResolverConfig::from_parts(None, vec![],
            NameServerConfigGroup::from(vec![nsc]),
        ),
        ResolverOpts::default(),
    );

    let (resolver_config, mut resolver_opts) = read_system_conf().unwrap();
    resolver_opts.positive_max_ttl = Some(Duration::from_secs(1));
    resolver_opts.negative_max_ttl = Some(Duration::from_secs(1));

    tracing::info!(resolver_config =?resolver_config, resolver_opts =?resolver_opts);

    let resolver = TokioAsyncResolver::tokio(resolver_config, resolver_opts);

    resolver.clear_cache();
    let resolved = resolver.lookup_ip("ton-liteserver-headless.ton-grpc-mainnet.svc.cluster.local").await?;

    for record in resolved {
        tracing::info!(record = ?record);
    }

    Ok(())
}
