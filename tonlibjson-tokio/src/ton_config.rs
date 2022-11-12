use reqwest::IntoUrl;

pub type TonConfig = String;

pub async fn load_ton_config<U: IntoUrl>(url: U) -> anyhow::Result<String> {
    let config = reqwest::get(url).await?.text().await?;

    Ok(config)
}

#[cfg(test)]
mod tests {
    use crate::ton_config::load_ton_config;

    #[tokio::test]
    async fn load_config_mainnet() {
        let url = "https://ton.org/global-config.json";

        let config = load_ton_config(url).await.unwrap();

        assert!(config.contains("\"@type\": \"config.global\""))
    }
}
