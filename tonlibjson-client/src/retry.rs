use std::sync::Arc;
use futures::future;
use tower::retry::budget::Budget;
use tower::retry::Policy;
use crate::balance::BalanceRequest;
use crate::session::SessionRequest;
use crate::session::SessionRequest::RunGetMethod;

#[derive(Clone)]
pub struct RetryPolicy {
    budget: Arc<Budget>
}

impl RetryPolicy {
    pub fn new(budget: Budget) -> Self {
        Self {
            budget: Arc::new(budget)
        }
    }
}

impl<E, Res> Policy<SessionRequest, Res, E> for RetryPolicy {
    type Future = future::Ready<Self>;

    fn retry(&self, _: &SessionRequest, result: Result<&Res, &E>) -> Option<Self::Future> {
        match result {
            Ok(_) => {
                self.budget.deposit();

                None
            }
            Err(_) => {
                match self.budget.withdraw() {
                    Ok(_) => Some(future::ready(self.clone())),
                    Err(_) => None
                }
            }
        }
    }

    fn clone_request(&self, req: &SessionRequest) -> Option<SessionRequest> {
        match req {
            SessionRequest::Atomic(req) => Some(SessionRequest::Atomic(req.with_new_id())),
            RunGetMethod { address, method, stack } => Some(
                RunGetMethod {address: address.clone(), method: method.clone(), stack: stack.clone()}
            )
        }
    }
}

impl<E, Res> Policy<BalanceRequest, Res, E> for RetryPolicy {
    type Future = future::Ready<Self>;

    fn retry(&self, _: &BalanceRequest, result: Result<&Res, &E>) -> Option<Self::Future> {
        match result {
            Ok(_) => {
                self.budget.deposit();

                None
            }
            Err(_) => {
                match self.budget.withdraw() {
                    Ok(_) => Some(future::ready(self.clone())),
                    Err(_) => None
                }
            }
        }
    }

    fn clone_request(&self, req: &BalanceRequest) -> Option<BalanceRequest> {
        <Self as tower::retry::Policy<SessionRequest, Res, E>>::clone_request(self, &req.request)
            .map(|inner|
                BalanceRequest::new(req.block.clone(), inner)
            )
    }
}
