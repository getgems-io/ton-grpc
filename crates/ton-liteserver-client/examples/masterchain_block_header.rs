use testcontainers_ton::LocalLiteServer;
use ton_liteserver_client::client::LiteServerClient;
use ton_liteserver_client::tl::{LiteServerGetBlockHeader, LiteServerGetMasterchainInfo};
use ton_liteserver_client::tlb::block_header::BlockHeader;
use ton_liteserver_client::tlb::merkle_proof::MerkleProof;
use toner::tlb::BoC;
use toner::tlb::bits::de::unpack_bytes;
use tower::ServiceExt;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt::init();

    let lite_server = LocalLiteServer::new().await?;
    let mut client =
        LiteServerClient::connect(lite_server.addr(), lite_server.server_key()).await?;

    let info = (&mut client)
        .oneshot(LiteServerGetMasterchainInfo::default())
        .await?;
    let header = (&mut client)
        .oneshot(LiteServerGetBlockHeader::new(info.last))
        .await?;

    let boc: BoC = unpack_bytes(&header.header_proof, ())?;
    let root = boc.single_root().unwrap();

    println!("root = {root:?}");

    let header: BlockHeader = root.parse_fully_as::<_, MerkleProof<_>>(())?;

    println!("header = {header:?}");

    Ok(())
}
