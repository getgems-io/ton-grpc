use std::time::Duration;
use tokio::time::interval;
use tower::{Service, ServiceExt};
use tower::discover::ServiceList;
use tower::limit::ConcurrencyLimit;
use tower::load::{CompleteOnResponse, Load, PeakEwma, PeakEwmaDiscover};
use tracing::info;
use tonlibjson_client::config::AppConfig;
use tonlibjson_client::cursor_client::CursorClient;
use tonlibjson_client::make::{CursorClientFactory, SessionClientFactory};
use tonlibjson_client::session::SessionClient;
use tonlibjson_client::ton_config::load_ton_config;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt::init();

    let app_config = AppConfig::from_env()?;
    let ton_config = load_ton_config(app_config.config_url).await?;

    let client = SessionClientFactory::default()
        .ready()
        .await?
        .call(ton_config)
        .await?;


    let mut client: CursorClient = CursorClientFactory::create(PeakEwma::new(client, Duration::from_secs(5), 500000.0, tower::load::CompleteOnResponse::default()));

    // info!("start seqno: {:?}, end seqno: {:?}",
    //     p.first_block().expect("must be synced").id.seqno,
    //     p.last_block().expect("must be synced").id.seqno
    // );

    let mut timer = interval(Duration::from_secs(5));

    loop {
        timer.tick().await;

        let current_block = client.load().last_block.map(|b| b.id.seqno);

        info!("current seqno: {:?}", current_block)
    }

    Ok(())
}
