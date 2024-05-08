use std::net::{Ipv4Addr, SocketAddrV4};
use std::time::Duration;
use base64::Engine;
use futures::{stream, StreamExt};
use tower::{ServiceBuilder, ServiceExt};
use adnl_tcp::client::ServerKey;
use ton_liteserver_client::client::LiteServerClient;
use ton_liteserver_client::tl::{LiteServerGetMasterchainInfo, LiteServerLookupBlock, TonNodeBlockId};

#[tokio::main]
async fn main() -> Result<(), tower::BoxError> {
    tracing_subscriber::fmt::init();

    let client = provided_client().await.expect("cannot connect");
    let mut svc = ServiceBuilder::new()
        .concurrency_limit(10)
        .timeout(Duration::from_secs(3))
        .service(client);

    let last = (&mut svc).oneshot(LiteServerGetMasterchainInfo {}).await?.unbox().last;

    let requests = stream::iter((1 .. last.seqno).rev())
        .map(|seqno| LiteServerLookupBlock { mode: 1, id: TonNodeBlockId { workchain: last.workchain, shard: last.shard, seqno }, lt: None, utime: None });

    let mut responses = svc.call_all(requests).unordered();

    while let Some(item) = responses.next().await.transpose().inspect_err(|e| tracing::error!(e))? {
        let item = item.unbox();
        tracing::info!(?item.id.seqno);
    }
    Ok(())
}

async fn provided_client() -> anyhow::Result<LiteServerClient> {
    let ip: i32 = -2018135749;
    let ip = Ipv4Addr::from(ip as u32);
    let port = 53312;
    let key: ServerKey = base64::engine::general_purpose::STANDARD.decode("aF91CuUHuuOv9rm2W5+O/4h38M3sRm40DtSdRxQhmtQ=")?.as_slice().try_into()?;

    tracing::info!("Connecting to {}:{} with key {:?}", ip, port, key);

    let client = LiteServerClient::connect(SocketAddrV4::new(ip, port), &key).await?;

    Ok(client)
}
