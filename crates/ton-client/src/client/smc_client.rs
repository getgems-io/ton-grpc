use crate::Client;
use crate::RequestHandler;
use ton_address::SmartContractAddress;
use ton_tower::request::RunGetMethod;
use ton_tower::response::{SmcRunResult, StackEntry};
use tower::ServiceExt;

impl<S> Client<S>
where
    S: RequestHandler<RunGetMethod>,
{
    pub async fn run_get_method(
        &mut self,
        address: &SmartContractAddress,
        method: &str,
        stack: Vec<StackEntry>,
    ) -> anyhow::Result<SmcRunResult> {
        let address = address.clone();
        let method = method.to_string();
        self.oneshot(RunGetMethod {
            address,
            method,
            stack,
        })
        .await
    }
}
