use adnl_tcp::client::ServerKey;
use base64::Engine;
use std::net::{Ipv4Addr, SocketAddrV4};
use std::time::Duration;
use ton_liteserver_client::client::LiteServerClient;
use ton_liteserver_client::tl::{LiteServerGetAllShardsInfo, LiteServerGetMasterchainInfo};
use toner::{
    tlb::bits::de::unpack_bytes,
    ton::boc::BoC
};
use tower::{ServiceBuilder, ServiceExt};
use ton_liteserver_client::tlb::shard_hashes::ShardHashes;

#[tokio::main]
async fn main() -> Result<(), tower::BoxError> {
    tracing_subscriber::fmt::init();

    let client = provided_client().await.expect("cannot connect");
    let mut svc = ServiceBuilder::new()
        .concurrency_limit(10)
        .timeout(Duration::from_secs(3))
        .service(client);

    let id = (&mut svc)
        .oneshot(LiteServerGetMasterchainInfo::default())
        .await?
        .last;
    let shards = (&mut svc)
        .oneshot(LiteServerGetAllShardsInfo { id })
        .await
        .unwrap();

    let boc: BoC = unpack_bytes(&shards.data)?;
    let root = boc.single_root().unwrap();
    let shards: ShardHashes = root.parse_fully().unwrap();

    for (workchain_id, shards) in shards.iter() {
        for shard in shards {
            println!("workchain_id = {}, shard_id = {:x}", workchain_id, shard.next_validator_shard);
        }
    }

    Ok(())
}

async fn provided_client() -> anyhow::Result<LiteServerClient> {
    let ip: i32 = -2018135749;
    let ip = Ipv4Addr::from(ip as u32);
    let port = 53312;
    let key: ServerKey = base64::engine::general_purpose::STANDARD
        .decode("aF91CuUHuuOv9rm2W5+O/4h38M3sRm40DtSdRxQhmtQ=")?
        .as_slice()
        .try_into()?;

    tracing::info!("Connecting to {}:{} with key {:?}", ip, port, key);

    let client = LiteServerClient::connect(SocketAddrV4::new(ip, port), &key).await?;

    Ok(client)
}
