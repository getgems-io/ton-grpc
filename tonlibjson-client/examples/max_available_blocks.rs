use futures::{stream, StreamExt};
use tower::{Service, ServiceExt};
use tracing::{error, info};
use tonlibjson_client::block::BlockIdExt;
use tonlibjson_client::config::AppConfig;
use tonlibjson_client::make::ClientFactory;
use tonlibjson_client::session::SessionRequest;
use tonlibjson_client::ton_config::load_ton_config;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt::init();

    let config = AppConfig::from_env()?;
    let config = load_ton_config(config.config_url).await?;

    stream::iter(&config.liteservers)
        .for_each_concurrent(None, |ls| {
            let config = config.clone();
            async move {
                let mut factory = ClientFactory::default();
                let svc = factory.ready().await.expect("err")
                    .call(config.with_liteserver(&ls)).await;

                match svc {
                    Err(e) => error!("error: {}", e),
                    Ok(svc) => {
                        info!("min_block: {:#?}", svc.get_ref().get_ref().min_block)
                    }
                }
            }
        }).await;

    Ok(())
}
