use std::time::Duration;
use tokio::time::interval;
use tower::{Service, ServiceExt};
use tower::load::{Load, PeakEwma};
use tracing::info;
use tonlibjson_client::config::AppConfig;
use tonlibjson_client::cursor_client::CursorClient;
use tonlibjson_client::make::{CursorClientFactory, ClientFactory};
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
        .call(ton_config)
        .await?;


    let mut client: CursorClient = CursorClientFactory::create(PeakEwma::new(client, Duration::from_secs(5), 500000.0, tower::load::CompleteOnResponse::default()));
    let mut timer = interval(Duration::from_secs(5));

    client.ready().await?;

    info!("client ready");

    for _ in 0.. 20 * 5 {
        timer.tick().await;

        let current_block = client.load().unwrap().last_block.id.seqno;

        info!("current seqno: {:?}", current_block);

        let info = client.ready().await?.call(SessionRequest::GetMasterchainInfo {}).await?;

        info!("{:?}", info);
    }

    Ok(())
}
