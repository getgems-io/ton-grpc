use futures::{stream, StreamExt};
use std::time::Duration;
use testcontainers_ton::LocalLiteServer;
use ton_liteserver_client::client::LiteServerClient;
use ton_liteserver_client::tl::{
    LiteServerGetMasterchainInfo, LiteServerLookupBlock, TonNodeBlockId,
};
use tower::{ServiceBuilder, ServiceExt};

#[tokio::main]
async fn main() -> Result<(), tower::BoxError> {
    tracing_subscriber::fmt::init();

    let lite_server = LocalLiteServer::new().await?;
    let client =
        LiteServerClient::connect(lite_server.get_addr(), lite_server.get_server_key()).await?;
    let mut svc = ServiceBuilder::new()
        .concurrency_limit(10)
        .timeout(Duration::from_secs(3))
        .service(client);

    let last = (&mut svc)
        .oneshot(LiteServerGetMasterchainInfo::default())
        .await?
        .last;

    let requests = stream::iter((1..last.seqno).rev()).map(|seqno| LiteServerLookupBlock {
        mode: 1,
        id: TonNodeBlockId {
            workchain: last.workchain,
            shard: last.shard,
            seqno,
        },
        lt: None,
        utime: None,
    });

    let mut responses = svc.call_all(requests).unordered();

    while let Some(item) = responses
        .next()
        .await
        .transpose()
        .inspect_err(|e| tracing::error!(e))?
    {
        tracing::info!(?item.id.seqno);
    }
    Ok(())
}
