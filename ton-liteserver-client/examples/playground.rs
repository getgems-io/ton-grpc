use adnl_tcp::client::ServerKey;
use base64::Engine;
use std::net::{Ipv4Addr, SocketAddrV4};
use ton_liteserver_client::client::LiteServerClient;
use ton_liteserver_client::tl::{
    LiteServerGetMasterchainInfo, LiteServerListBlockTransactions, LiteServerLookupBlock,
    TonNodeBlockId, True,
};
use tower::ServiceExt;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt::init();

    let mut client = provided_client().await?;

    let last = (&mut client)
        .oneshot(LiteServerGetMasterchainInfo::default())
        .await?
        .last;
    tracing::info!(?last);

    let partial_block_id = TonNodeBlockId {
        workchain: last.workchain,
        shard: last.shard,
        seqno: last.seqno - 200000,
    };
    let block_id = (&mut client)
        .oneshot(LiteServerLookupBlock {
            mode: 1,
            id: partial_block_id,
            lt: None,
            utime: None,
        })
        .await?;
    tracing::info!(?block_id);

    let txs = (&mut client)
        .oneshot(LiteServerListBlockTransactions {
            id: last,
            mode: 15,
            count: 4,
            after: None,
            reverse_order: None,
            want_proof: Some(True {}),
        })
        .await?;
    for tx in txs.ids {
        tracing::info!(?tx)
    }

    let txs = (&mut client)
        .oneshot(LiteServerListBlockTransactions {
            id: block_id.id.clone(),
            mode: 1,
            count: 4,
            after: None,
            reverse_order: None,
            want_proof: None,
        })
        .await?;
    for tx in txs.ids {
        tracing::info!(?tx)
    }

    let txs = (&mut client)
        .oneshot(LiteServerListBlockTransactions {
            id: block_id.id,
            mode: 1,
            count: 4,
            after: None,
            reverse_order: Some(True {}),
            want_proof: None,
        })
        .await?;
    for tx in txs.ids {
        tracing::info!(?tx)
    }

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
