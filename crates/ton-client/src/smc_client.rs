use async_trait::async_trait;

use crate::{SmcRunResult, StackEntry};

#[async_trait]
pub trait SmcClient: Clone + Send + Sync + 'static {
    async fn run_get_method(
        &self,
        address: &str,
        method: &str,
        stack: Vec<StackEntry>,
    ) -> anyhow::Result<SmcRunResult>;
}
