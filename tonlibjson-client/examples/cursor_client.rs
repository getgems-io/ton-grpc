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

    for ls in ton_config.liteservers.clone() {
        let client = ClientFactory::default()
            .ready()
            .await?
            .call(ton_config.clone().with_liteserver(&ls))
            .await?;

        let mut client: CursorClient = CursorClientFactory::create(PeakEwma::new(client, Duration::from_secs(5), 500000.0, tower::load::CompleteOnResponse::default()));

        client.ready().await?;

        let metrics = client.load().unwrap();

        error!(seqno = metrics.first_block.0.id.seqno, lt = metrics.first_block.0.start_lt, "master start");
        error!(seqno = metrics.last_block.0.id.seqno, lt = metrics.last_block.0.end_lt, "master end");

        error!(seqno = metrics.first_block.1.id.seqno, lt = metrics.first_block.1.start_lt, "work start");
        error!(seqno = metrics.last_block.1.id.seqno, lt = metrics.last_block.1.end_lt, "work end");
    }

    Ok(())
}
