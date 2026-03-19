use adnl_tcp::client::ServerKey;
use base64::Engine;
use std::net::{Ipv4Addr, SocketAddrV4};
use testcontainers_ton::genesis::Genesis;
use testcontainers_ton::lite_server::LiteServer;
use ton_liteserver_client::client::LiteServerClient;
use ton_liteserver_client::tl::{LiteServerGetBlockHeader, LiteServerGetMasterchainInfo};
use ton_liteserver_client::tlb::block_header::BlockHeader;
use ton_liteserver_client::tlb::merkle_proof::MerkleProof;
use toner::tlb::bits::de::unpack_bytes;
use toner::tlb::BoC;
use tower::ServiceExt;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt::init();

    use testcontainers::runners::AsyncRunner;
    let genesis = Genesis::default().start().await?;

    println!("genesis = {:?}", genesis.get_host().await?);
    let mut config = vec![];
    genesis
        .copy_file_from("/usr/share/data/global.config.json", &mut config)
        .await?;
    let liteserver = LiteServer::new(config).start().await?;
    println!("liteserver started");

    let mut client = provided_client(liteserver.get_host_port_ipv4(30004).await?).await?;

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

async fn provided_client(port: u16) -> anyhow::Result<LiteServerClient> {
    let ip: i32 = 2130706433; // 127.0.0.1
    let ip = Ipv4Addr::from(ip as u32);
    let key: ServerKey = base64::engine::general_purpose::STANDARD
        .decode("Wha42OjSNvDaHOjhZhUZu0zW/+wu/+PaltND/a0FbuI=")?
        .as_slice()
        .try_into()?;

    tracing::info!("Connecting to {}:{} with key {:?}", ip, port, key);

    let client = LiteServerClient::connect(SocketAddrV4::new(ip, port), key).await?;

    Ok(client)
}
