use testcontainers_ton::LocalLiteServer;
use ton_liteserver_client::client::LiteServerClient;
use ton_liteserver_client::tl::{LiteServerGetMasterchainInfo, LiteServerListBlockTransactionsExt};
use toner::tlb::BoC;
use toner::tlb::bits::de::unpack_bytes;
use tower::ServiceExt;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt::init();

    let lite_server = LocalLiteServer::new().await?;
    let mut client =
        LiteServerClient::connect(lite_server.addr(), lite_server.server_key()).await?;

    for _ in 1..=10 {
        let info = (&mut client)
            .oneshot(LiteServerGetMasterchainInfo::default())
            .await?;
        println!("last block: seqno={}", info.last.seqno);

        let response = (&mut client)
            .oneshot(LiteServerListBlockTransactionsExt {
                id: info.last,
                mode: 0,
                count: 100,
                after: None,
                reverse_order: None,
                want_proof: None,
            })
            .await?;

        let boc: BoC = unpack_bytes(&response.transactions, ())?;
        for root in boc.into_roots() {
            println!("transaction: {:?}", root);
        }

        tokio::time::sleep(std::time::Duration::from_secs(2)).await;
    }

    Ok(())
}
