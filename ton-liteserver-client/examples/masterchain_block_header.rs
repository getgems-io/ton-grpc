use adnl_tcp::client::ServerKey;
use base64::Engine;
use std::net::{Ipv4Addr, SocketAddrV4};
use ton_liteserver_client::client::LiteServerClient;
use ton_liteserver_client::tl::{LiteServerGetBlockHeader, LiteServerGetMasterchainInfo};
use ton_liteserver_client::tlb::merkle_proof::MerkleProof;
use toner::tlb::bits::de::unpack_bytes;
use toner::ton::boc::BoC;
use tower::ServiceExt;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt::init();

    let mut client = provided_client().await?;

    let info = (&mut client)
        .oneshot(LiteServerGetMasterchainInfo::default())
        .await?;
    let header = (&mut client)
        .oneshot(LiteServerGetBlockHeader::new(info.last))
        .await?;

    let boc: BoC = unpack_bytes(&header.header_proof)?;
    let root = boc.single_root().unwrap();

    println!("root = {:?}", root);

    let header: MerkleProof = root.parse_fully()?;

    println!("header = {:?}", header);

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
