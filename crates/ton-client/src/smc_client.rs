use crate::{SmcRunResult, StackEntry};
use async_trait::async_trait;
use ton_address::SmartContractAddress;

#[async_trait]
pub trait SmcClient: Clone + Send + Sync + 'static {
    async fn run_get_method(
        &self,
        address: &SmartContractAddress,
        method: &str,
        stack: Vec<StackEntry>,
    ) -> anyhow::Result<SmcRunResult>;
}
