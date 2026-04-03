use anyhow::anyhow;
use std::time::Duration;
use testcontainers_ton::LocalLiteServer;
use ton_liteserver_client::client::LiteServerClient;
use ton_liteserver_client::tl::{
    LiteServerGetAllShardsInfo, LiteServerGetBlockHeader, LiteServerGetMasterchainInfo,
    TonNodeBoxedBlockIdExt,
};
use ton_liteserver_client::tlb::block_header::BlockHeader;
use ton_liteserver_client::tlb::merkle_proof::MerkleProof;
use ton_liteserver_client::tlb::shard_hashes::ShardHashes;
use toner::tlb::BoC;
use toner::tlb::bits::de::{unpack_bytes, unpack_bytes_fully};
use tower::{ServiceBuilder, ServiceExt};

#[tokio::main]
async fn main() -> Result<(), tower::BoxError> {
    tracing_subscriber::fmt::init();

    let lite_server = LocalLiteServer::new().await?;
    let client = LiteServerClient::connect(lite_server.addr(), lite_server.server_key()).await?;
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

    let boc: BoC = unpack_bytes(&shards.data, ())?;
    let root = boc
        .single_root()
        .ok_or_else(|| anyhow!("single root expected"))?;
    let shards: ShardHashes = root.parse_fully(())?;

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

            let boc: BoC = unpack_bytes_fully(&header.header_proof, ())?;
            let header: BlockHeader = boc
                .single_root()
                .unwrap()
                .parse_fully_as::<_, MerkleProof<_>>(())?;

            println!("header = {header:?}");
        }
    }

    Ok(())
}
