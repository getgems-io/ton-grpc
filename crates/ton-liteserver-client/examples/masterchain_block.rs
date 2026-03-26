use testcontainers_ton::LocalLiteServer;
use ton_liteserver_client::client::LiteServerClient;
use ton_liteserver_client::tl::{LiteServerGetBlock, LiteServerGetMasterchainInfo};
use ton_liteserver_client::tlb::block::Block;
use toner::tlb::BoC;
use toner::tlb::bits::de::unpack_bytes;
use tower::ServiceExt;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt::init();

    let lite_server = LocalLiteServer::new().await?;
    let mut client =
        LiteServerClient::connect(lite_server.get_addr(), lite_server.get_server_key()).await?;

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
