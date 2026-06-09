mod request;

use crate::route::Error as RouteError;
use futures::FutureExt;
use futures::future::BoxFuture;
use std::sync::Arc;
use std::time::Duration;
use tokio_retry::strategy::{FibonacciBackoff, jitter};
use tower::retry::Policy;
use tower::retry::budget::{Budget, TpsBudget};

#[derive(Clone)]
pub struct RetryPolicy {
    budget: Arc<TpsBudget>,
    backoff: FibonacciBackoff,
}

pub(crate) trait Retryable {
    const IS_RETRYABLE: bool;
}

impl RetryPolicy {
    pub fn new(budget: TpsBudget, first_delay_millis: u64, max_delay: Duration) -> Self {
        metrics::describe_counter!(
            "ton_retry_budget_withdraw_success",
            "Number of withdraws that were successful"
        );
        metrics::describe_counter!(
            "ton_retry_budget_withdraw_fail",
            "Number of withdraws that were unsuccessful"
        );

        let retry_strategy = FibonacciBackoff::from_millis(first_delay_millis).max_delay(max_delay);

        Self {
            budget: Arc::new(budget),
            backoff: retry_strategy,
        }
    }
}

impl<T, Res> Policy<T, Res, tower::BoxError> for RetryPolicy
where
    T: Clone + Retryable,
{
    type Future = BoxFuture<'static, ()>;

    fn retry(
        &mut self,
        _: &mut T,
        result: &mut Result<Res, tower::BoxError>,
    ) -> Option<Self::Future> {
        if !T::IS_RETRYABLE {
            return None;
        }

        match result {
            Ok(_) => {
                self.budget.deposit();

                None
            }
            Err(e) => {
                if let Some(RouteError::RouteUnknown) = e.downcast_ref::<RouteError>() {
                    return None;
                }

                let request_type: &str = std::any::type_name::<T>();

                if self.budget.withdraw() {
                    metrics::counter!("ton_retry_budget_withdraw_success", "request_type" => request_type).increment(1);

                    Some({
                        let millis = self.backoff.by_ref().map(jitter).next().unwrap();

                        tokio::time::sleep(millis).boxed()
                    })
                } else {
                    metrics::counter!("ton_retry_budget_withdraw_fail", "request_type" => request_type).increment(1);

                    None
                }
            }
        }
    }

    fn clone_request(&mut self, req: &T) -> Option<T> {
        if T::IS_RETRYABLE {
            Some(req.clone())
        } else {
            None
        }
    }
}
