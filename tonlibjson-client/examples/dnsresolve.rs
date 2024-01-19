use tokio_stream::StreamExt;
use tonlibjson_client::dns_discover::DnsResolverDiscover;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt::init();

    let mut discover = DnsResolverDiscover::new("ton-liteserver-headless.ton-grpc-mainnet.svc.cluster.local");

    while let Ok(record) = discover.next().await.unwrap() {
        tracing::info!(record = ?record);
    }

    Ok(())
}
