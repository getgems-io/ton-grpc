use std::time::Duration;
use tower::{Service, ServiceExt};
use tower::load::PeakEwma;
use tracing::info;
use tonlibjson_client::block::GetMasterchainInfo;
use tonlibjson_client::config::AppConfig;
use tonlibjson_client::cursor_client::CursorClient;
use tonlibjson_client::make::{CursorClientFactory, ClientFactory};

use tonlibjson_client::ton_config::load_ton_config;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt::init();

    let app_config = AppConfig::from_env()?;
    let ton_config = load_ton_config(app_config.config_url).await?;

    let mut alive = 0;
    let mut dead = 0;

    for ls in ton_config.liteservers.clone() {
        let Ok(client) = ClientFactory
            .ready()
            .await?
            .call(ton_config.clone().with_liteserver(&ls))
            .await else {
            dead += 1;
            continue
        };

        alive += 1;

        let mut client: CursorClient = CursorClientFactory::create(ls.id(),PeakEwma::new(client, Duration::from_secs(5), 500000.0, tower::load::CompleteOnResponse::default()));

        ServiceExt::<GetMasterchainInfo>::ready(&mut client).await?;

        // let first_block = client.take_first_block().unwrap();
        // let last_block = client.take_last_block().unwrap();
        //
        // info!(seqno = first_block.0.id.seqno, lt = first_block.0.start_lt, "master start");
        // info!(seqno = last_block.0.id.seqno, lt = last_block.0.end_lt, "master end");
        //
        // info!(seqno = first_block.1.id.seqno, lt = first_block.1.start_lt, "work start");
        // info!(seqno = last_block.1.id.seqno, lt = last_block.1.end_lt, "work end");
        //
        // let contains = last_block.0.id.seqno - first_block.0.id.seqno;
        // let d = contains * 12 / 60 / 60 / 24;
        //
        // info!(contains = contains, d = d, "data");
    }

    info!(dead = dead, alive = alive);

    Ok(())
}
