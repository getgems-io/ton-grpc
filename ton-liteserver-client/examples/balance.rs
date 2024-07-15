use base64::Engine;
use futures::{stream, StreamExt};
use std::net::SocketAddrV4;
use std::time::Duration;
use tokio::time::Instant;
use ton_client_util::discover::{read_ton_config_from_url_stream, LiteServerDiscover};
use ton_client_util::router::balance::Balance;
use ton_client_util::service::shared::SharedLayer;
use ton_liteserver_client::client::{Error, LiteServerClient};
use ton_liteserver_client::make::MakeClient;
use ton_liteserver_client::tl::{
    LiteServerGetMasterchainInfo, LiteServerLookupBlock, TonNodeBlockId,
};
use ton_liteserver_client::tracked_client::TrackedClient;
use tower::discover::Change;
use tower::reconnect::Reconnect;
use tower::{BoxError, ServiceBuilder, ServiceExt};

#[tokio::main]
async fn main() -> Result<(), tower::BoxError> {
    tracing_subscriber::fmt::init();

    let discovery = LiteServerDiscover::new(read_ton_config_from_url_stream(
        "https://ton.org/global-config.json".parse()?,
        tokio::time::interval_at(Instant::now(), Duration::from_secs(30)),
    ))
    .then(|change| async {
        match change {
            Ok(Change::Insert(k, v)) => {
                let liteservers = v.liteservers;
                let Some(ls) = liteservers.first().cloned() else {
                    unreachable!()
                };

                let mut secret_key: [u8; 32] = [0; 32];
                base64::engine::general_purpose::STANDARD
                    .decode_slice(&ls.id.key, &mut secret_key[..])?;
                let addr: SocketAddrV4 = ls.into();

                let client = ServiceBuilder::new()
                    .layer_fn(TrackedClient::new)
                    .concurrency_limit(1000)
                    .map_err(|e: BoxError| match e.downcast::<Error>() {
                        Ok(e) => *e,
                        Err(_) => Error::Elapsed,
                    })
                    .timeout(Duration::from_secs(10))
                    .layer(SharedLayer)
                    .map_err(|e: BoxError| match e.downcast::<Error>() {
                        Ok(e) => *e,
                        Err(e) => Error::Connection(e.to_string()),
                    })
                    .service(Reconnect::new::<LiteServerClient, ()>(
                        MakeClient::new(addr, secret_key),
                        (),
                    ));

                anyhow::Ok(Change::Insert(k, client))
            }
            Ok(Change::Remove(k)) => Ok(Change::Remove(k)),
            Err(_) => unreachable!(),
        }
    });

    let mut svc = ServiceBuilder::new()
        .concurrency_limit(100000)
        .service(Balance::new(discovery.boxed()));

    let last = (&mut svc)
        .oneshot(LiteServerGetMasterchainInfo::default())
        .await?
        .last;

    tracing::info!("Last block: {}", last.seqno);

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
    while let Some(item) = responses.next().await {
        match item {
            Ok(response) => tracing::info!(?response.id.seqno),
            Err(e) => tracing::error!(?e),
        }
    }
    Ok(())
}
