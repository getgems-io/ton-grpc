use adnl_tcp::client::ServerKey;
use base64::Engine;
use std::net::{Ipv4Addr, SocketAddrV4};
use ton_liteserver_client::client::LiteServerClient;
use ton_liteserver_client::tl::{LiteServerGetBlock, LiteServerGetMasterchainInfo};
use toner::tlb::bits::de::unpack_bytes;
use toner::tlb::BoC;
use tower::ServiceExt;
use ton_liteserver_client::tlb::block::Block;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt::init();

    let mut client = provided_client().await?;

    let info = (&mut client)
        .oneshot(LiteServerGetMasterchainInfo::default())
        .await?;
    let response = (&mut client)
        .oneshot(LiteServerGetBlock::new(info.last))
        .await?;

    let boc: BoC = unpack_bytes(&response.data, ())?;
    let root = boc.single_root().unwrap();

    let block: Block = root.parse_fully(())?;
    assert_eq!(response.id.seqno, block.info.seq_no as i32);

    println!("block = {block:?}");

    Ok(())
}

async fn provided_client() -> anyhow::Result<LiteServerClient> {
    let ip: i32 = 1091956407;
    let ip = Ipv4Addr::from(ip as u32);
    let port = 16351;
    let key: ServerKey = base64::engine::general_purpose::STANDARD
        .decode("Mf/JGvcWAvcrN3oheze8RF/ps6p7oL6ifrIzFmGQFQ8=")?
        .as_slice()
        .try_into()?;

    tracing::info!("Connecting to {}:{} with key {:?}", ip, port, key);

    let client = LiteServerClient::connect(SocketAddrV4::new(ip, port), key).await?;

    Ok(client)
}
