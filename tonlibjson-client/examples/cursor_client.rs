use std::time::Duration;
use tokio::time::interval;
use tower::{Service, ServiceExt};
use tower::load::{Load, PeakEwma};
use tracing::info;
use tonlibjson_client::block::{AccountTransactionId, BlocksGetShards, BlocksGetTransactions};
use tonlibjson_client::config::AppConfig;
use tonlibjson_client::cursor_client::CursorClient;
use tonlibjson_client::make::{CursorClientFactory, ClientFactory};
use tonlibjson_client::request::Requestable;
use tonlibjson_client::session::SessionRequest;

use tonlibjson_client::ton_config::load_ton_config;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt::init();

    let app_config = AppConfig::from_env()?;
    let ton_config = load_ton_config(app_config.config_url).await?;

    let client = ClientFactory::default()
        .ready()
        .await?
        .call(ton_config.with_liteserver(ton_config.liteservers.first().take().unwrap()))
        .await?;


    let mut client: CursorClient = CursorClientFactory::create(PeakEwma::new(client, Duration::from_secs(5), 500000.0, tower::load::CompleteOnResponse::default()));
    let mut timer = interval(Duration::from_secs(5));

    client.ready().await?;

    info!("client ready");

    for _ in 0.. 20 * 5 {
        timer.tick().await;

        let current_block = client.load().unwrap().first_block;

        info!(chain = current_block.id.workchain, seqno = current_block.id.seqno, "seqno");

        let last_block = client
            .ready()
            .await?
            .call(SessionRequest::FindFirstBlock { chain_id: 0 })
            .await;

        info!(last_block =? last_block, "last_block")
    }

    Ok(())
}
