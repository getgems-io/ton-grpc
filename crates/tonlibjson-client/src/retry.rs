use crate::block::{RawSendMessage, RawSendMessageReturnHash};
use crate::error::Error;
use futures::future::BoxFuture;
use futures::FutureExt;
use std::sync::Arc;
use std::time::Duration;
use tokio_retry::strategy::{jitter, FibonacciBackoff};
use ton_client_util::router::route::Error as RouterError;
use tower::retry::budget::{Budget, TpsBudget};
use tower::retry::Policy;

#[derive(Clone)]
pub struct RetryPolicy {
    budget: Arc<TpsBudget>,
    backoff: FibonacciBackoff,
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

impl<Res, E> Policy<RawSendMessageReturnHash, Res, E> for RetryPolicy {
    type Future = BoxFuture<'static, ()>;

    fn retry(
        &mut self,
        _: &mut RawSendMessageReturnHash,
        _: &mut Result<Res, E>,
    ) -> Option<Self::Future> {
        None
    }

    fn clone_request(&mut self, _: &RawSendMessageReturnHash) -> Option<RawSendMessageReturnHash> {
        None
    }
}

impl<Res, E> Policy<RawSendMessage, Res, E> for RetryPolicy {
    type Future = BoxFuture<'static, ()>;

    fn retry(&mut self, _: &mut RawSendMessage, _: &mut Result<Res, E>) -> Option<Self::Future> {
        None
    }

    fn clone_request(&mut self, _: &RawSendMessage) -> Option<RawSendMessage> {
        None
    }
}

impl<T: Clone, Res> Policy<T, Res, tower::BoxError> for RetryPolicy {
    type Future = BoxFuture<'static, ()>;

    fn retry(
        &mut self,
        _: &mut T,
        result: &mut Result<Res, tower::BoxError>,
    ) -> Option<Self::Future> {
        match result {
            Ok(_) => {
                self.budget.deposit();

                None
            }
            Err(e) => {
                if e.is::<Error>() {
                    let downcast_err: &Error = e.downcast_ref().unwrap();
                    if matches!(downcast_err, Error::Route(RouterError::RouteUnknown)) {
                        return None;
                    }
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
        Some(req.clone())
    }
}
