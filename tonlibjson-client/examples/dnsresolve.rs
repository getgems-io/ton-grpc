use std::net::{IpAddr, SocketAddr};
use std::str::FromStr;
use std::time::Duration;
use tokio_stream::StreamExt;
use trust_dns_resolver::config::{NameServerConfig, NameServerConfigGroup, Protocol, ResolverConfig, ResolverOpts};
use trust_dns_resolver::{Name, TokioAsyncResolver};
use trust_dns_resolver::proto::rr::RecordType;
use trust_dns_resolver::system_conf::read_system_conf;
use tonlibjson_client::dns_discover::DnsResolverDiscover;
use tonlibjson_client::ton::{TonClient, TonClientBuilder};

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt::init();

    let mut discover = DnsResolverDiscover::new("ton-liteserver-headless.ton-grpc-mainnet.svc.cluster.local");

    while let Ok(record) = discover.next().await.unwrap() {
        tracing::info!(record = ?record);
    }

    Ok(())
}
