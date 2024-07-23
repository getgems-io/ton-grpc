use crate::block::{RawSendMessage, RawSendMessageReturnHash};
use crate::error::Error;
use futures::future::BoxFuture;
use futures::FutureExt;
use std::sync::Arc;
use std::time::Duration;
use tokio_retry::strategy::{jitter, FibonacciBackoff};
use ton_client_util::router::route::Error as RouterError;
use tower::retry::budget::Budget;
use tower::retry::Policy;

#[derive(Clone)]
pub struct RetryPolicy {
    budget: Arc<Budget>,
    backoff: FibonacciBackoff,
}

impl RetryPolicy {
    pub fn new(budget: Budget, first_delay_millis: u64, max_delay: Duration) -> Self {
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
    type Future = BoxFuture<'static, Self>;

    fn retry(&self, _: &RawSendMessageReturnHash, _: Result<&Res, &E>) -> Option<Self::Future> {
        None
    }

    fn clone_request(&self, _: &RawSendMessageReturnHash) -> Option<RawSendMessageReturnHash> {
        None
    }
}

impl<Res, E> Policy<RawSendMessage, Res, E> for RetryPolicy {
    type Future = BoxFuture<'static, Self>;

    fn retry(&self, _: &RawSendMessage, _: Result<&Res, &E>) -> Option<Self::Future> {
        None
    }

    fn clone_request(&self, _: &RawSendMessage) -> Option<RawSendMessage> {
        None
    }
}

impl<T: Clone, Res> Policy<T, Res, tower::BoxError> for RetryPolicy {
    type Future = BoxFuture<'static, Self>;

    fn retry(&self, _: &T, result: Result<&Res, &tower::BoxError>) -> Option<Self::Future> {
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

                match self.budget.withdraw() {
                    Ok(_) => {
                        metrics::counter!("ton_retry_budget_withdraw_success", "request_type" => request_type).increment(1);

                        Some({
                            let mut pol = self.clone();

                            async move {
                                let millis = pol.backoff.by_ref().map(jitter).next().unwrap();

                                tokio::time::sleep(millis).await;

                                pol
                            }
                            .boxed()
                        })
                    }
                    Err(_) => {
                        metrics::counter!("ton_retry_budget_withdraw_fail", "request_type" => request_type).increment(1);

                        None
                    }
                }
            }
        }
    }

    fn clone_request(&self, req: &T) -> Option<T> {
        Some(req.clone())
    }
}
