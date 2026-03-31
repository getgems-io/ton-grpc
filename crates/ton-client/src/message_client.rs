use async_trait::async_trait;

#[async_trait]
pub trait MessageClient: Clone + Send + Sync + 'static {
    async fn send_message(&self, message: &str) -> anyhow::Result<()>;

    async fn send_message_returning_hash(&self, message: &str) -> anyhow::Result<String>;
}
