use adnl_tcp::client::ServerKey;
use base64::Engine;
use futures::{stream, StreamExt, TryStreamExt};
use std::convert::Infallible;
use std::net::{Ipv4Addr, SocketAddrV4};
use std::str::FromStr;
use std::time::Duration;
use tokio::time::Instant;
use ton_client_util::discover::config::{LiteServerId, TonConfig};
use ton_client_util::discover::{read_ton_config_from_url_stream, LiteServerDiscover};
use ton_liteserver_client::balance::Balance;
use ton_liteserver_client::client::LiteServerClient;
use ton_liteserver_client::tl::{
    LiteServerGetMasterchainInfo, LiteServerLookupBlock, TonNodeBlockId,
};
use ton_liteserver_client::tracked_client::TrackedClient;
use tower::discover::Change;
use tower::{ServiceBuilder, ServiceExt};
use url::Url;

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
                let client = LiteServerClient::connect(ls.into(), &secret_key).await?;
                let client = TrackedClient::new(client);

                anyhow::Ok(Change::Insert(k, client))
            }
            Ok(Change::Remove(k)) => Ok(Change::Remove(k)),
            Err(_) => unreachable!(),
        }
    });

    let svc = Balance::new(discovery.boxed());
    let mut svc = ServiceBuilder::new()
        .concurrency_limit(10000)
        .service(svc);

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
