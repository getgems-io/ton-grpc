use adnl_tcp::client::ServerKey;
use base64::Engine;
use std::net::{Ipv4Addr, SocketAddrV4};
use std::time::Duration;
use anyhow::anyhow;
use ton_liteserver_client::client::LiteServerClient;
use ton_liteserver_client::tl::{LiteServerGetAllShardsInfo, LiteServerGetBlockHeader, LiteServerGetMasterchainInfo, TonNodeBoxedBlockIdExt};
use ton_liteserver_client::tlb::shard_hashes::ShardHashes;
use toner::{tlb::bits::de::unpack_bytes, ton::boc::BoC};
use toner::tlb::bits::de::unpack_bytes_fully;
use tower::{ServiceBuilder, ServiceExt};
use ton_liteserver_client::tlb::merkle_proof::MerkleProof;

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
        .await?;

    let boc: BoC = unpack_bytes(&shards.data)?;
    let root = boc.single_root().ok_or_else(|| anyhow!("single root expected"))?;
    let shards: ShardHashes = root.parse_fully()?;

    for (workchain_id, shards) in shards.iter() {
        for shard in shards {
            println!(
                "workchain_id = {}, shard_id = {:x}",
                workchain_id, shard.next_validator_shard
            );

            let block_id = TonNodeBoxedBlockIdExt {
                workchain: *workchain_id as i32,
                shard: shard.next_validator_shard as i64,
                seqno: shard.seq_no as i32,
                root_hash: shard.root_hash,
                file_hash: shard.file_hash,
            };

            let header = (&mut svc)
                .oneshot(LiteServerGetBlockHeader::new(block_id))
                .await?;

            let boc: BoC = unpack_bytes_fully(header.header_proof)?;
            let header: MerkleProof = boc.single_root().unwrap().parse_fully()?;

            println!("header = {:?}", header);
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

    let client = LiteServerClient::connect(SocketAddrV4::new(ip, port), key).await?;

    Ok(client)
}
