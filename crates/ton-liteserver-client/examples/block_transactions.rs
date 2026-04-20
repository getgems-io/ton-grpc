use testcontainers_ton::LocalLiteServer;
use ton_liteserver_client::client::LiteServerClient;
use ton_liteserver_client::tl::{LiteServerGetBlock, LiteServerGetMasterchainInfo};
use ton_liteserver_client::tlb::block::Block;
use ton_liteserver_client::tlb::transaction::Transaction;
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
    println!("last block: seqno={}", info.last.seqno);

    let response = (&mut client)
        .oneshot(LiteServerGetBlock::new(info.last))
        .await?;

    let boc: BoC = unpack_bytes(&response.data, ())?;
    let block: Block = boc.single_root().unwrap().parse_fully(())?;

    println!(
        "block: seqno={}, gen_utime={}",
        block.info.seq_no, block.info.gen_utime
    );
    println!(
        "account_blocks: {} accounts",
        block.extra.account_blocks.0.len()
    );

    for (_account_key, account_block) in &block.extra.account_blocks.0 {
        let addr = hex::encode(account_block.account_addr);
        for (tx_key, tx_cell) in &account_block.transactions {
            let tx: Transaction = tx_cell.parse_fully(())?;
            println!(
                "  tx: key={} addr={}, lt={}, now={}",
                tx_key, addr, tx.lt, tx.now
            );
        }
    }

    Ok(())
}
